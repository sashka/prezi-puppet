use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;
use std::{fs, thread};

use clap::Parser;
use headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption;
use headless_chrome::{Browser, LaunchOptions};
use log::debug;
use printpdf::image_crate::codecs::png::PngDecoder;
use printpdf::{ColorSpace, Image, ImageTransform, Mm, PdfDocument};

/// So called "Prezi to PDF" for macOS
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Target PDF file name
    #[arg(short = 'O', value_name = "PDF")]
    target: String,

    /// Prezi url
    url: String,
}

const BROWSER_WINDOW: (u32, u32) = (1024, 768);
const BROWSER_BORDER: (u32, u32) = (0, 130);
const CHROMIUM_PATH: &str = "/Applications/Chromium.app/Contents/MacOS/Chromium";

fn main() {
    // let _ = simplelog::SimpleLogger::init(simplelog::LevelFilter::Debug, simplelog::Config::default());
    let args = Args::parse();
    println!("{:?}", args);

    let result = browse_prezi(&args.url, &args.target);
    exit_with_code(&result);
}

fn browse_prezi(url: &str, target_pdf: &str) -> anyhow::Result<()> {
    debug!("Processing `{}`", url);

    let tmp_dir = tempfile::tempdir()?;

    let options = LaunchOptions::default_builder()
        .path(Some(PathBuf::from_str(CHROMIUM_PATH)?))
        .window_size(Some(BROWSER_WINDOW))
        .enable_gpu(true)
        .headless(false)
        .idle_browser_timeout(Duration::from_secs(60))
        .build()
        .expect("Couldn't find appropriate Chrome binary.");
    let browser = Browser::new(options)?;
    let tab = browser.new_tab()?;
    tab.navigate_to(url)?;

    // Wait for network/javascript/dom to make "Present" button available and click it.
    tab.wait_for_element("div.viewer-common-info-overlay-center-block")?.click()?;
    thread::sleep(Duration::from_secs(3));

    // Wait until spinner "Loading content..." is gone.
    while tab.find_element("div.webgl-viewer-navbar-spinner-wrapper").is_ok() {
        debug!("Waiting until spinner is gone...");
        thread::sleep(Duration::from_secs(3));
    }

    let title = tab.find_element("title")?.get_inner_text()?;
    debug!("Title: `{}`", &title);

    let mut pages = Vec::new();

    // Wait for the "next" button, wait 3s more and click
    loop {
        let n = pages.len();

        debug!("Taking screenshot {}", n);
        let screenshot = tab.capture_screenshot(CaptureScreenshotFormatOption::Png, Some(90), None, false)?;

        let file_path = tmp_dir.path().join(format!("{}.png", n));
        debug!("Saving screenshot {} -> {:?}", n, file_path.as_path());
        fs::write(&file_path, screenshot)?;

        let p = file_path.as_path().to_owned();
        pages.push(p);

        if let Ok(next) = tab.wait_for_element("button.webgl-viewer-navbar-button-next") {
            next.click()?;
            thread::sleep(Duration::from_secs(3));
        } else {
            break;
        }
    }

    debug!("Rendering PDF: `{}`", &target_pdf);
    combine_pdf(target_pdf, &title, &pages)?;
    debug!("Done");
    Ok(())
}

fn combine_pdf<P>(target_name: &str, title: &str, pages: &[P]) -> anyhow::Result<()>
where
    P: AsRef<Path>,
{
    let pdf = PdfDocument::empty(title);

    for p in pages.iter() {
        let (page, layer_index) = pdf.add_page(
            pixels_to_mm((BROWSER_WINDOW.0 - BROWSER_BORDER.0) * 2),
            pixels_to_mm((BROWSER_WINDOW.1 - BROWSER_BORDER.1) * 2),
            "",
        );
        let layer = pdf.get_page(page).get_layer(layer_index);
        let mut img_file = fs::File::open(p)?;
        let decoder = PngDecoder::new(&mut img_file)?;
        let mut image = Image::try_from(decoder)?;
        remove_alpha(&mut image);
        image.add_to_layer(
            layer.clone(),
            ImageTransform {
                translate_x: Some(Mm(0.0)),
                translate_y: Some(Mm(0.0)),
                rotate: None,
                scale_x: Some(1.0),
                scale_y: Some(1.0),
                dpi: Some(300.0),
            },
        );
    }

    pdf.save(&mut BufWriter::new(fs::File::create(target_name).unwrap())).unwrap();
    Ok(())
}

#[inline(always)]
fn pixels_to_mm(px: u32) -> Mm {
    Mm((px as f64) * 0.084666667)
}

// Removes alpha channel.
// Assumes source image data pixel has RGBA structure, which is true for PNG.
fn remove_alpha(img: &mut Image) {
    img.image.color_space = ColorSpace::Rgb;
    img.image.image_data = img
        .image
        .image_data
        .chunks(4)
        .map(|rgba| [rgba[0], rgba[1], rgba[2]])
        .collect::<Vec<[u8; 3]>>()
        .concat();
}

fn exit_with_code(result: &anyhow::Result<()>) {
    match result {
        Ok(_) => {
            std::process::exit(0);
        }
        Err(err) => {
            eprintln!("Error: {:?}", err);
            std::process::exit(255);
        }
    }
}
