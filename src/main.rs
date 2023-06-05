use headless_chrome::{Browser, LaunchOptions};
use headless_chrome::protocol::cdp::Page;

fn main() {
   let _ = browse_prezi("B8pzpL1lp2mU7e7ULtc0");
}

fn browse_prezi(id: &str) -> anyhow::Result<()> {
    let options = LaunchOptions::default_builder()
        .headless(false)
        .build()
        .expect("Couldn't find appropriate Chrome binary.");
    let browser = Browser::new(options)?;
    let tab = browser.new_tab()?;
    tab.navigate_to(format!("https://prezi.com/view/{}/", id).as_str())?;

    // Wait for network/javascript/dom to make the search-box available
    // and click it.
    tab.wait_for_element("div.viewer-common-info-overlay-center-block")?.click()?;

    Ok(())
}