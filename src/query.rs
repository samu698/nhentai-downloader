use std::num::NonZeroU32;
use std::str::FromStr;

use anyhow::{Context, Result};

use reqwest::{Url, Client};
use reqwest::header;

use scraper::{Html, Selector};

use crate::{SortType, ctx};

pub enum QueryResult {
    QueryList(QueryInfo, Vec<u32>),
    Gallery(u32)
}

pub struct QueryInfo {
    sort: SortType,
    query: String,
    pages: NonZeroU32,
}

impl QueryInfo {
    pub fn pages(&self) -> NonZeroU32 { self.pages }

    fn query_url(query: &str, sort: SortType, page: NonZeroU32) -> String {
        let sort = match sort {
            SortType::Recent => "",
            SortType::Popular => "&sort=popular",
            SortType::PopularWeek => "&sort=popular-week",
            SortType::PopularToday => "&sort=popular-today"
        };
        format!("https://nhentai.net/search/?q={query}&page={page}{sort}")
    }

    fn parse_gallery_path(path: &str) -> Result<u32> {
        let path = path.trim_matches('/');
        let Some(("g", code)) = path.split_once('/') else {
            anyhow::bail!("Path is not to a gallery")
        };
        code.parse()
            .with_context(ctx!("What? Gallery page code is not a number"))
    }

    fn read_query_page(document: Html, query: &str, page: NonZeroU32) -> Vec<u32> {
        let selector = Selector::parse("a.cover").unwrap();
        document
            .select(&selector)
            .enumerate()
            .filter_map(|(idx, e)| match e.attr("href") {
                Some(href) => Some((idx, href)),
                None => {
                    log::warn!("Missing link to gallery #{}, at page {page}, query: \"{query}\"", idx + 1);
                    None
                }
            })
            .filter_map(|(idx, href)| match Self::parse_gallery_path(href) {
                Ok(code) => Some(code),
                Err(_) => {
                    log::warn!("Invalid link to gallery #{}, at page {page}, query: \"{query}\"", idx + 1);
                    None
                }
            })
            .collect()
    }

    pub async fn load(client: &Client, query: &str, sort: SortType, page: NonZeroU32) -> Result<QueryResult> {
        let url = Self::query_url(query, sort, page);

        log::trace!("Connecting to query page: {url}");
        let res = client.get(&url)
            .send().await
            .with_context(ctx!("Failed to retrive query page at {url}"))?
            .error_for_status()
            .with_context(ctx!("Received error from nhentai at {url}"))?;

        if res.status().is_redirection() {
            log::trace!("Query page at {url} is a redirection, opening gallery");

            let location = res.headers().get(header::LOCATION)
                .with_context(ctx!("What? Redirection reply is missing the location header at {url}"))?
                .to_str()
                .with_context(ctx!("What? Cannot convert location header to string"))?;

            let location = match Url::parse(location) {
                Ok(url) => url.path().to_string(),
                Err(_) => location.to_string()
            };
            
            Self::parse_gallery_path(&location)
                .with_context(ctx!("What? Redirect from query is not a gallery, Redirect: {location}, Query: {query}"))
                .map(QueryResult::Gallery)
        } else {
            log::trace!("Parsing query page at {url}");

            let text = res.text().await
                .with_context(ctx!("Failed to retrive query page contents, URL: {url}"))?;
            let document = Html::parse_document(&text);

            // Find the last page number
            let last_page_selector = Selector::parse("a.last").unwrap();
            let mut last_page_iter = document.select(&last_page_selector);
            let last_page = last_page_iter.next()
                .with_context(ctx!("What? Missing last page button on query page, URL: {url}"))?;
            if last_page_iter.next().is_some() {
                log::warn!("What? Multiple last page buttons on query page, using first, URL: {url}");
            }

            let last_href = last_page.attr("href")
                .with_context(ctx!("What? Missing href on last page button, URL: {url}"))?;

            let base = Url::from_str("https://nhentai.net").unwrap();
            let last_href = base.join(last_href)
                .with_context(ctx!("What? Invalid url for last page button, href: {last_href}, URL: {url}"))?;

            let (_, last_page) = last_href.query_pairs()
                .find(|(key, _)| key == "page")
                .with_context(ctx!("What? Missing page for last page button, href: {last_href}, URL: {url}"))?;

            let last_page: NonZeroU32 = last_page.parse()
                .with_context(ctx!("What? Cannot parse last page number"))?;

            log::trace!("Query: \"{query}\" has {last_page} pages");

            let s = Self {
                query: query.to_string(),
                sort,
                pages: last_page,
            };

            let galleries = Self::read_query_page(document, query, page);
            log::trace!("Found {} galleries on page {page} of query {query}", galleries.len());
            Ok(QueryResult::QueryList(s, galleries))
        }
    }

    pub async fn load_page(&self, client: &Client, page: NonZeroU32) -> Result<Vec<u32>> {
        let url = Self::query_url(&self.query, self.sort, page);

        log::trace!("Connecting to query page: {url}");
        let text = client.get(&url)
            .send().await
            .with_context(ctx!("Failed to retrive query page at {url}"))?
            .error_for_status()
            .with_context(ctx!("Received error from nhentai at {url}"))?
            .text().await
            .with_context(ctx!("Failed to retrive query page contents, URL: {url}"))?;

        let document = Html::parse_document(&text);
        let galleries = Self::read_query_page(document, &self.query, page);
        log::trace!("Found {} galleries on page {page} of query {}", galleries.len(), &self.query);
        Ok(galleries)
    }
}
