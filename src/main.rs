use std::num::NonZeroU32;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

use reqwest::Client;
use reqwest::redirect::Policy as RedirectPolicy;

mod gallery;
use gallery::Gallery;
mod logging;
mod query;
use query::{QueryInfo, QueryResult};

#[macro_export]
macro_rules! ctx {
    ($($arg:tt)+) => {
        || format!($($arg)+)
    };
}

#[derive(clap::Parser)]
#[command(name = "nhentai-downloader", version, author, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    action: ActionType,
    #[arg(short = 'v', long, verbatim_doc_comment)]
    /// Verbose output
    verbose: bool,
    #[arg(short = 'x', long, verbatim_doc_comment)]
    /// Overwrite already existing pages
    overwrite: bool,
    #[arg(long, verbatim_doc_comment)]
    #[arg(conflicts_with = "overwrite")]
    /// Don't check for missing pages in already downloaded galleries.
    ///
    /// By default when downloading a gallery that is already in the output directory this program
    /// will check if all pages are present and try to download the missing ones, this flag
    /// disables this behavior.
    no_check_missing_pages: bool,
    #[arg(short = 'p', long, verbatim_doc_comment)]
    /// Path to output directory
    path: PathBuf,
}

#[derive(clap::Subcommand)]
#[command(disable_help_subcommand = true, flatten_help = true)]
enum ActionType {
    Single(SingleCli),
    Query(QueryCli),
}

#[derive(clap::Args)]
#[command(disable_help_flag = true)]
/// Single gallery download mode
struct SingleCli {
    #[arg(verbatim_doc_comment)]
    /// Id of the gallery to download
    id: u32,
}

#[derive(clap::Args)]
#[command(disable_help_flag = true)]
/// Query download mode
struct QueryCli {
    #[arg(verbatim_doc_comment)]
    /// Query string to fetch galleries
    ///
    /// - By default this will download all the galleries of first page of the query.
    /// - If the query refers to a single gallery (e.g. "#12345") only that gallery will be
    ///     downloaded, other flags will be ignored.
    /// - You can find the query syntax here: https://nhentai.net/info/
    query: String, // TODO: verbatim_doc_comment
    #[arg(short = 's', long, verbatim_doc_comment)]
    #[arg(value_enum, default_value_t)]
    /// Query sort order
    sort: SortType,
    #[arg(short = 'f', long, verbatim_doc_comment)]
    #[arg(default_value = "1")]
    /// First page to download (inclusive)
    ///
    /// - If this number is bigger than the total available pages nothing will be downloaded.
    first_page: NonZeroU32,
    #[arg(short = 'l', long, verbatim_doc_comment)]
    #[arg(conflicts_with = "count")]
    /// Last page to download (inclusive)
    ///
    /// - Must be bigger than first-page.
    /// - If this number is bigger than the available pages all pages will be downloaded.
    last_page: Option<NonZeroU32>,
    #[arg(short = 'n', long, verbatim_doc_comment)]
    /// Number of pages to download (from first)
    ///
    /// - Set to 0 to download all pages.
    /// - If this number is bigger than the available pages all pages will be downloaded.
    count: Option<u32>,
}

/// Possible sort orders for a query
#[derive(clap::ValueEnum, Clone, Copy, Default)]
enum SortType {
    #[default]
    Recent,
    Popular,
    PopularWeek,
    PopularToday,
}

struct App {
    args: Cli,
    client: Client,
}

impl App {
    fn new(args: Cli) -> Result<Self> {
        let client = Client::builder()
            .redirect(RedirectPolicy::custom(|attempt| {
                // HACK: because there is no way to set the redirect policy of a client after building
                // it, we use this function to ignore redirects when redirecting away from the search
                // page allowing QueryInfo to detect it
                let [.., prev] = attempt.previous() else { unreachable!() };
                if prev.path().trim_matches('/') == "search" {
                    attempt.stop()
                } else if attempt.previous().len() > 10 {
                    attempt.error("Too many redirects")
                } else {
                    attempt.follow()
                }
            }))
            .build()
            .with_context(ctx!("Cannot build http client"))?;

        Ok(Self { args, client })
    }

    async fn run(&self) -> Result<()> {
        match self.args.action {
            ActionType::Query(ref q) => self.download_query(q).await,
            ActionType::Single(SingleCli { id }) => self.download_gallery(id, None).await,
        }?;

        Ok(())
    }

    async fn download_gallery(&self, id: u32, progress: Option<(usize, usize)>) -> Result<()> {
        let gallery = Gallery::load(&self.client, id).await
            .with_context(ctx!("Failed to load gallery {id}"))?;

        match progress {
            Some((pos, end)) => log::info!("({pos}/{end}) id: {id} [{}] pages: {}", gallery.title.pretty, gallery.pages()),
            None => log::info!("Downloading gallery: {id} [{}] pages: {}", gallery.title.pretty, gallery.pages()),
        }

        gallery.download(&self.client, &self.args.path, self.args.overwrite, !self.args.no_check_missing_pages).await
            .with_context(ctx!("Failed to download gallery {id}"))
    }

    async fn download_query(&self, query: &QueryCli) -> Result<()> {
        let query_res = QueryInfo::load(&self.client, &query.query, query.sort, query.first_page).await
            .with_context(ctx!("Failed to load query `{}`", query.query))?;

        let (query_info, galleries) = match query_res {
            QueryResult::Gallery(id) => {
                log::info!("The provided query points to a single gallery");
                return self.download_gallery(id, None).await;
            }
            QueryResult::QueryList(i, g) => (i, g)
        };

        if query_info.pages() < query.first_page {
            anyhow::bail!("The first page must be less that the number of pages (it's {})", query_info.pages());
        }

        log::info!("Found {} pages available for query `{}`", query_info.pages(), query.query);

        let last_page = match (query.last_page, query.count) {
            (Some(last), _) => last,
            (_, Some(0)) => query_info.pages(),
            (_, Some(count)) => query.first_page.checked_add(count - 1).unwrap_or(query_info.pages()),
            (None, None) => unreachable!()
        };
        let last_page = last_page.min(query_info.pages());

        log::info!(">>> ({}/{last_page}) Downloading query page #{}", query.first_page, query.first_page);
        let gallery_count = galleries.len();
        for (i, gallery) in galleries.into_iter().enumerate() {
            if let Err(e) = self.download_gallery(gallery, Some((i + 1, gallery_count))).await {
                log::warn!("Failed to download gallery: {gallery}\nError: {e:?}");
            }
        }

        for page in (query.first_page.get()..=last_page.get()).skip(1) {
            // SAFETY: None of the numbers between two non-zero numbers are zero.
            let page = unsafe { NonZeroU32::new_unchecked(page) };

            let galleries = match query_info.load_page(&self.client, page).await {
                Ok(g) => g,
                Err(e) => {
                    log::warn!("Failed to download query page: {page}\nError: {e:?}");
                    continue;
                }
            };

            log::info!(">>> ({page}/{last_page}) Downloading query page #{page}");
            let gallery_count = galleries.len();
            for (i, gallery) in galleries.into_iter().enumerate() {
                if let Err(e) = self.download_gallery(gallery, Some((i + 1, gallery_count))).await {
                    log::warn!("Failed to download gallery: {gallery}\nError: {e:?}");
                }
            }
        }

        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let args = Cli::parse();
    logging::init(args.verbose);

    let app = match App::new(args) {
        Ok(app) => app,
        Err(e) => {
            log::error!("{e:?}");
            return;
        }
    };

    match app.run().await {
        Ok(_) => {}
        Err(e) => log::error!("{e:?}")
    }
}
