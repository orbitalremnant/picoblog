use anyhow::{anyhow, Result};
use chrono::{DateTime, NaiveDate, Utc};
use gray_matter::{engine::YAML, Matter};
use html_escape::encode_text;
use once_cell::sync::Lazy;
use pulldown_cmark::{html, Options, Parser};
use regex::Regex;
use resvg::{tiny_skia, usvg};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tera::Tera;
use url::Url;
use walkdir::WalkDir;

// --- Statically Compiled Regexes ---
static RE_FIRST_URL: Lazy<Regex> = Lazy::new(|| Regex::new(r"https?://[^\s()<>]+").unwrap());
static RE_BODY_TAGS: Lazy<Regex> = Lazy::new(|| Regex::new(r"#(\p{L}[\p{L}\p{N}-]*)").unwrap());
static RE_FILENAME_DATE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(\d{4})[^[:alnum:]](\d{2})[^[:alnum:]](\d{2})[^[:alnum:]](.+)").unwrap()
});
static RE_HTML_RESOURCES: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?:src|href)=["'](.*?)["']"#).unwrap());

/// Represents a social media share link.
#[derive(Debug, Serialize)]
pub struct ShareLink {
    /// The name of the provider (e.g., "X", "Facebook").
    pub provider_name: String,
    /// The generated URL for sharing.
    pub url: String,
}

/// Represents a single parsed article.
#[derive(Debug, Serialize)]
pub struct Article {
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub created: Option<NaiveDate>,
    pub modified: Option<NaiveDate>,
    pub link_url: Option<String>,
    pub html_content: String,
    pub content: String,
    pub slug: String,
    pub share_links: Vec<ShareLink>,
}

/// Represents the optional frontmatter fields in a Markdown file.
#[derive(Debug, Deserialize, Default)]
struct Frontmatter {
    title: Option<String>,
    description: Option<String>,
    tags: Option<Vec<String>>,
    created: Option<String>,
    modified: Option<String>,
    link_url: Option<String>,
}

/// Represents an entry in the client-side search index.
#[derive(Debug, Serialize)]
struct SearchEntry<'a> {
    title: &'a str,
    description: &'a str,
    tags: &'a [String],
    html_content: &'a str,
    slug: &'a str,
}

/// Extracts the first absolute URL from a string.
fn extract_first_url(content: &str) -> Option<String> {
    RE_FIRST_URL.find(content).map(|m| m.as_str().to_string())
}

/// Extracts hashtags (e.g., #rust, #你好) from a string, with Unicode support.
fn extract_body_tags(content: &str) -> Vec<String> {
    RE_BODY_TAGS
        .captures_iter(content)
        .map(|cap| cap[1].to_string())
        .collect()
}

/// Converts a `SystemTime` to a `NaiveDate`.
fn system_time_to_naive_date(st: SystemTime) -> NaiveDate {
    let datetime: DateTime<Utc> = st.into();
    datetime.date_naive()
}

/// Extracts a title and optional date from a filename (e.g., "2024-10-26-my-post.md").
fn extract_metadata_from_path(path: &Path) -> (String, Option<NaiveDate>) {
    let file_stem = path.file_stem().unwrap().to_string_lossy();
    let (date_opt, title_slug) = if let Some(caps) = RE_FILENAME_DATE.captures(&file_stem) {
        let year = caps.get(1).unwrap().as_str();
        let month = caps.get(2).unwrap().as_str();
        let day = caps.get(3).unwrap().as_str();
        let date_str = format!("{}-{}-{}", year, month, day);
        let title_part = caps.get(4).unwrap().as_str();
        (
            NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").ok(),
            title_part.to_string(),
        )
    } else {
        (None, file_stem.to_string())
    };
    let title = title_slug.replace(['-', '_'], " ");
    (title, date_opt)
}

/// Validates that all `src` and `href` attributes in an HTML string point to absolute URLs.
fn validate_resource_urls(html_content: &str, source_file: &Path) -> Result<()> {
    for cap in RE_HTML_RESOURCES.captures_iter(html_content) {
        let url_str = &cap[1];
        // Skip empty URLs, page-local anchors, or data URIs
        if url_str.is_empty() || url_str.starts_with('#') || url_str.starts_with("data:") {
            continue;
        }
        // Use the `url` crate to robustly check if the URL is absolute.
        if Url::parse(url_str).is_err() {
            return Err(anyhow!("Validation failed for file '{}': Found relative or invalid resource link: '{}'. All resource links (src/href) must be absolute URLs.", source_file.display(), url_str));
        }
    }
    Ok(())
}

/// Generates a list of social sharing links based on provider templates.
///
/// Note: This function performs several string allocations for URL encoding and
/// template replacement. While this is acceptable for a static site generator
/// that runs infrequently, in a high-performance context, this could be optimized.
fn generate_share_links(
    providers: &[(String, String)],
    url: &Option<String>,
    title: &str,
    text: &str,
    tags: &[String],
) -> Vec<ShareLink> {
    let url_to_encode = url.as_deref().unwrap_or("");
    let url_encoded = urlencoding::encode(url_to_encode);
    let title_encoded = urlencoding::encode(title);
    let text_encoded = urlencoding::encode(text);

    // Format tags as "#tag1 #tag2".
    // Spaces inside a specific tag are replaced with underscores (e.g. "my tag" -> "#my_tag").
    let tags_string = tags
        .iter()
        .map(|t| format!("#{}", t.replace(' ', "_")))
        .collect::<Vec<_>>()
        .join(" ");
    let tags_encoded = urlencoding::encode(&tags_string);

    providers
        .iter()
        .map(|(provider_name, template)| {
            let final_url = template
                .replace("{URL}", &url_encoded)
                .replace("{TITLE}", &title_encoded)
                .replace("{TEXT}", &text_encoded)
                .replace("{TAGS}", &tags_encoded);

            ShareLink {
                provider_name: provider_name.clone(),
                url: final_url,
            }
        })
        .collect()
}

/// Parses a file (Markdown or plain text) into an `Article`.
fn parse_file(path: &Path, share_providers: &[(String, String)]) -> Result<Article> {
    let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    match extension {
        "md" => parse_markdown_file(path, share_providers),
        "txt" => parse_text_file(path, share_providers),
        _ => Err(anyhow!("Unsupported file type: {}", path.display())),
    }
}

/// Parses a plain text file into an `Article`.
fn parse_text_file(path: &Path, share_providers: &[(String, String)]) -> Result<Article> {
    let (path_title, path_date) = extract_metadata_from_path(path);
    let content = fs::read_to_string(path)?;
    let metadata = fs::metadata(path)?;

    // Note: File creation time is not reliable across all platforms (especially Unix).
    // We prioritize modified time as a more consistent fallback.
    let modified_date = system_time_to_naive_date(metadata.modified()?);
    let created_date = path_date.or_else(|| metadata.created().ok().map(system_time_to_naive_date));

    let escaped_content = encode_text(&content);
    let html_content = format!("<p>{}</p>", escaped_content.replace('\n', "<br>"));
    let slug = path.file_stem().unwrap().to_string_lossy().to_string();

    let link_url = extract_first_url(&content);
    let tags = extract_body_tags(&content);
    let share_links =
        generate_share_links(share_providers, &link_url, &path_title, &content, &tags);

    Ok(Article {
        title: path_title,
        description: String::new(),
        tags,
        html_content,
        slug,
        content,
        created: created_date.or(Some(modified_date)),
        modified: Some(modified_date),
        link_url,
        share_links,
    })
}

/// Parses a Markdown file with optional YAML frontmatter into an `Article`.
fn parse_markdown_file(path: &Path, share_providers: &[(String, String)]) -> Result<Article> {
    let (path_title, path_date) = extract_metadata_from_path(path);
    let content = fs::read_to_string(path)?;
    let metadata = fs::metadata(path)?;

    // Note: File creation time is not reliable across all platforms (especially Unix).
    // We prioritize modified time as a more consistent fallback.
    let file_modified_date = system_time_to_naive_date(metadata.modified()?);
    let file_created_date = metadata.created().ok().map(system_time_to_naive_date);

    let matter = Matter::<YAML>::new();
    let result = matter.parse(&content).unwrap();

    let frontmatter: Frontmatter = result
        .data
        .map(|d: gray_matter::Pod| d.deserialize())
        .transpose()?
        .unwrap_or_default();

    let title = frontmatter.title.unwrap_or(path_title);

    let created_date = frontmatter
        .created
        .and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok())
        .or(path_date)
        .or(file_created_date)
        .or(Some(file_modified_date));

    let modified_date = frontmatter
        .modified
        .and_then(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok())
        .unwrap_or(file_modified_date);

    let markdown_content = result.content;

    let link_url = frontmatter
        .link_url
        .or_else(|| extract_first_url(&markdown_content));

    let mut tags = frontmatter.tags.unwrap_or_default();
    tags.extend(extract_body_tags(&markdown_content));
    tags.sort();
    tags.dedup();

    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(&markdown_content, options);
    let mut html_content = String::new();
    html::push_html(&mut html_content, parser);
    validate_resource_urls(&html_content, path)?;

    let slug = path.file_stem().unwrap().to_string_lossy().to_string();
    let share_links =
        generate_share_links(share_providers, &link_url, &title, &markdown_content, &tags);

    Ok(Article {
        title,
        description: frontmatter.description.unwrap_or_default(),
        tags,
        html_content,
        slug,
        content: markdown_content,
        created: created_date,
        modified: Some(modified_date),
        link_url,
        share_links,
    })
}

///
/// Finds and parses all supported content files (`.md`, `.txt`) in the given source paths.
///
/// # Arguments
/// * `source_paths` - A slice of `PathBuf` pointing to directories or files to scan.
/// * `share_providers` - A slice of tuples containing share provider names and URL templates.
///
/// # Returns
/// A `Result` containing a vector of `Article`s, sorted by creation date (descending),
/// or an error if scanning or parsing fails.
///
pub fn find_and_parse_articles(
    source_paths: &[PathBuf],
    share_providers: &[(String, String)],
) -> Result<Vec<Article>> {
    let mut articles = Vec::new();
    for source_path in source_paths {
        for entry in WalkDir::new(source_path).into_iter().filter_map(Result::ok) {
            if let Some(ext) = entry.path().extension().and_then(|s| s.to_str()) {
                if ext == "md" || ext == "txt" {
                    println!("Processing: {}", entry.path().display());
                    match parse_file(entry.path(), share_providers) {
                        Ok(article) => articles.push(article),
                        Err(e) => eprintln!("-> Skipping file {}: {}", entry.path().display(), e),
                    }
                }
            }
        }
    }
    articles.sort_by(|a, b| b.created.cmp(&a.created));
    Ok(articles)
}

/// Generates a set of favicons from the first character of the blog title.
fn generate_favicons(title: &str, output_dir: &Path) -> Result<()> {
    let initial = title
        .chars()
        .next()
        .unwrap_or('●')
        .to_uppercase()
        .to_string();

    let svg_string = format!(
        r#"<svg viewBox="0 0 100 100" xmlns="http://www.w3.org/2000/svg">
            <circle cx="50" cy="50" r="48" fill="white" stroke="rgba(0,0,0,0.1)" stroke-width="2"/>
            <text x="50%" y="50%" dy=".35em" text-anchor="middle" font-family="sans-serif" font-size="100" font-weight="bold" fill="black">{}</text>
        </svg>"#,
        initial
    );

    // Save favicon.svg
    fs::write(output_dir.join("favicon.svg"), &svg_string)?;

    // Prepare for rendering
    let opt = usvg::Options {
        default_size: usvg::Size::from_wh(100.0, 100.0).unwrap(),
        ..Default::default()
    };
    let mut fontdb = usvg::fontdb::Database::new();
    fontdb.load_system_fonts();
    let rtree = usvg::Tree::from_str(&svg_string, &opt)?;

    // Render and save apple-touch-icon.png (180x180)
    let mut pixmap_180 = tiny_skia::Pixmap::new(180, 180).unwrap();
    let scale_180 = 180.0 / 100.0;
    let transform_180 = tiny_skia::Transform::from_scale(scale_180, scale_180);
    resvg::render(&rtree, transform_180, &mut pixmap_180.as_mut());
    pixmap_180.save_png(output_dir.join("apple-touch-icon.png"))?;

    // Render 32x32 PNG for the .ico file
    let mut pixmap_32 = tiny_skia::Pixmap::new(32, 32).unwrap();
    let scale_32 = 32.0 / 100.0;
    let transform_32 = tiny_skia::Transform::from_scale(scale_32, scale_32);
    resvg::render(&rtree, transform_32, &mut pixmap_32.as_mut());

    // Create and save favicon.ico
    let mut icon_dir = ico::IconDir::new(ico::ResourceType::Icon);
    let image = ico::IconImage::from_rgba_data(32, 32, pixmap_32.data().to_vec());
    icon_dir.add_entry(ico::IconDirEntry::encode(&image)?);
    let file = fs::File::create(output_dir.join("favicon.ico"))?;
    icon_dir.write(file)?;

    println!("Generated favicons from title: '{}'", title);
    Ok(())
}

///
/// Generates the final static site files.
///
/// This function creates the output directory, generates favicons, builds the
/// search index, and renders the final `index.html` file.
///
/// # Arguments
/// * `articles` - A vector of `Article`s to include in the site.
/// * `settings` - A `tera::Context` containing site-wide settings (e.g., title, description).
/// * `output_dir` - The `Path` where the generated site files will be saved.
///
/// # Returns
/// A `Result` indicating success or failure.
///
pub fn generate_site(
    articles: Vec<Article>,
    settings: &tera::Context,
    output_dir: &Path,
) -> Result<()> {
    fs::create_dir_all(output_dir)?;

    // Extract title and generate favicons
    if let Some(settings_map) = settings.get("settings").and_then(|v| v.as_object()) {
        if let Some(title_val) = settings_map.get("title") {
            if let Some(title) = title_val.as_str() {
                generate_favicons(title, output_dir)?;
            }
        }
    }

    // Generate search index
    let search_index: Vec<SearchEntry> = articles
        .iter()
        .map(|a| SearchEntry {
            title: &a.title,
            description: &a.description,
            tags: &a.tags,
            html_content: &a.html_content,
            slug: &a.slug,
        })
        .collect();
    let search_json = serde_json::to_string(&search_index)?;
    fs::write(output_dir.join("search_index.json"), search_json)?;

    // Render final HTML
    let mut context = settings.clone();
    context.insert("articles", &articles);
    let template = include_str!("../templates/index.html");
    let final_html = Tera::one_off(template, &context, true)?;
    fs::write(output_dir.join("index.html"), final_html)?;

    println!(
        "\nSite generated successfully in '{}'!",
        output_dir.display()
    );
    Ok(())
}
