pub mod search;
pub mod web;
#[allow(unused_imports)]
pub use search::{SearchResult, format_search_results, web_search};
#[allow(unused_imports)]
pub use web::{ScrapeResult, scrape_url, scrape_urls};
