# NHentai-Downloder

This tool allows you to downalod nhentai galleries, you can download a single
gallery or multiple galleries mathing a query on nhentai.

## Usage

```
Usage: nhentai-downloader --path <PATH> single <ID>
       nhentai-downloader --path <PATH> query [OPTIONS] <QUERY>

Options:
  -v, --verbose
          Verbose output

  -x, --overwrite
          Overwrite already existing pages

      --no-check-missing-pages
          Don't check for missing pages in already downloaded galleries.
          
          By default when downloading a gallery that is already in the output directory this program
          will check if all pages are present and try to download the missing ones, this flag
          disables this behavior.

  -p, --path <PATH>
          Path to output directory

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version

nhentai-downloader --path <PATH> single:
Single gallery download mode
  <ID>
          Id of the gallery to download

nhentai-downloader --path <PATH> query:
Query download mode
  -s, --sort <SORT>
          Query sort order
          
          [default: recent]
          [possible values: recent, popular, popular-week, popular-today]

  -f, --first-page <FIRST_PAGE>
          First page to download (inclusive)
          
          - If this number is bigger than the total available pages nothing will be downloaded.
          
          [default: 1]

  -l, --last-page <LAST_PAGE>
          Last page to download (inclusive)
          
          - Must be bigger than first-page.
          - If this number is bigger than the available pages all pages will be downloaded.

  -n, --count <COUNT>
          Number of pages to download (from first)
          
          - Set to 0 to download all pages.
          - If this number is bigger than the available pages all pages will be downloaded.

  <QUERY>
          Query string to fetch galleries
          
          - By default this will download all the galleries of first page of the query.
          - If the query refers to a single gallery (e.g. "#12345") only that gallery will be
              downloaded, other flags will be ignored.
          - You can find the query syntax here: https://nhentai.net/info/
```

If you want to select more specific galleries you can leverage the 
[nhentai query syntax](https://nhentai.net/info)

Example queries:
- English galleries: `language:english` 
- Full color and tanlines tags: `tag:"full color" tag:"tanlines"`
- Not netorare: `-tag:netorare`
- Between 10 and 20 pages: `pages:>=10 pages:<21`
- Older than 30 days `uploaded:>30d` (units `h`, `d`, `w`, `m`, `h`)
- Mangas: `categories:manga`
- Complex query: `tags:inseki pages:28 uploaded:>7y uploaded:<86m -language:chinese`

## Output format
Downloaded files will be placed in the selected output folder, each gallery will
be in its own folder with a name equal to the gallery id.

Each gallery folder contais the downaloded pages numbered with a single number,
and a `gallery.json` file that contains info about the gallery.

Example folder structure:
```
out
├── 111111
│   ├── 1.jpg
│   ├── 2.jpg
│   ├── ...
│   ├── 40.jpg
│   └── gallery.json
└── 111112
    ├── 1.webp
    ├── 2.webp
    ├── ...
    ├── 121.webp
    └── gallery.json
```
