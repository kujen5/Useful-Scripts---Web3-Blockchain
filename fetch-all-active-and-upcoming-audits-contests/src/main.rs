use anyhow::{Ok, Result}; // used for good error handling
//use std::io::Read; // needed to call read_to_string on the response body
use reqwest::Client; // main HTTP client
use scraper::{Html, Selector}; // parse HTML



#[tokio::main] // used to perform async requests. like asyncio.run() in python
async fn main() -> Result<()>{
    let client = Client::new();
    let html=client
    .get("https://cantina.xyz/opportunities/bounties")
    .send() // send the request
    .await?
    .text()
    .await?;
// now res contains the entire HTML page
    let document=Html::parse_document(&html); // convert html into parsed document tree
    let card_selector = Selector::parse("a.chakra-stack.css-1gysfy9").unwrap();
    let title_selector = Selector::parse("h2.chakra-heading").unwrap();
    let org_selector = Selector::parse("p.chakra-text.css-1efsra7").unwrap();
    let amount_selector = Selector::parse("p.chakra-text.css-q35gqn").unwrap();
    for card in document.select(&card_selector) {
        let title = card
            .select(&title_selector)
            .next()
            .map(|t| t.text().collect::<Vec<_>>().join(""))
            .unwrap_or_default();

        let org = card
            .select(&org_selector)
            .next()
            .map(|o| o.text().collect::<Vec<_>>().join(""))
            .unwrap_or_default();

        let amount = card
            .select(&amount_selector)
            .next()
            .map(|a| a.text().collect::<Vec<_>>().join(""))
            .unwrap_or_default();

        println!("Title: {}\nOrganization: {}\nAmount: {}\n", title, org, amount);
    }

    
    Ok(())
}
