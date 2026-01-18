use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, PartialEq, Debug)]
struct Address {
    lines: String,
    city: String,
    state: String,
    postal: u64,
}

#[derive(Deserialize, Serialize, PartialEq, Debug)]
struct Person {
    given: String,
    family: String,
    address: Address,
}

#[derive(Deserialize, Serialize, PartialEq, Debug)]
struct ProductItem {
    sku: String,
    quantity: u64,
    description: String,
    // Price has decimals in YAML; parse as f64
    price: f64,
}

#[derive(Deserialize, Serialize, PartialEq, Debug)]
struct InvoiceDoc {
    invoice: u64,
    date: String,
    #[serde(rename = "bill-to")]
    bill_to: Person,
    #[serde(rename = "ship-to")]
    ship_to: Person,
    product: Vec<ProductItem>,
    tax: f64,
    total: f64,
    comments: String,
}

#[test]
fn y_ugm3_invoice_example_parse_invoice_ignored_due_to_anchor_alias_handling() -> anyhow::Result<()>
{
    let yaml = r#"---
!<tag:clarkevans.com,2002:invoice>
invoice: 34843
date   : 2001-01-23
bill-to: &id001
    given  : Chris
    family : Dumars
    address:
        lines: |
            458 Walkman Dr.
            Suite #292
        city    : Royal Oak
        state   : MI
        postal  : 48046
ship-to: *id001
product:
    - sku         : BL394D
      quantity    : 4
      description : Basketball
      price       : 450.00
    - sku         : BL4438H
      quantity    : 1
      description : Super Hoop
      price       : 2392.00
tax  : 251.42
total: 4443.52
comments:
    Late afternoon is best.
    Backup contact is Nancy
    Billsmer @ 338-4338.
    "#;

    let doc: InvoiceDoc = serde_saphyr::from_str(yaml)?;

    // Basic header fields
    assert_eq!(doc.invoice, 34843);
    assert_eq!(doc.date, "2001-01-23");

    // bill-to person and address
    assert_eq!(doc.bill_to.given, "Chris");
    assert_eq!(doc.bill_to.family, "Dumars");
    assert_eq!(doc.bill_to.address.city, "Royal Oak");
    assert_eq!(doc.bill_to.address.state, "MI");
    assert_eq!(doc.bill_to.address.postal, 48046);
    assert_eq!(doc.bill_to.address.lines, "458 Walkman Dr.\nSuite #292\n");

    // ship-to is an alias to bill-to; check a couple of fields
    assert_eq!(doc.ship_to.address.city, "Royal Oak");
    assert_eq!(doc.ship_to.address.state, "MI");

    // products
    assert_eq!(doc.product.len(), 2);
    assert_eq!(doc.product[0].sku, "BL394D");
    assert_eq!(doc.product[0].description, "Basketball");
    assert!((doc.product[0].price - 450.00).abs() < 1e-9);
    assert_eq!(doc.product[1].quantity, 1);
    assert_eq!(doc.product[1].description, "Super Hoop");
    assert!((doc.product[1].price - 2392.00).abs() < 1e-9);

    // totals and comments
    assert!((doc.tax - 251.42).abs() < 1e-9);
    assert!((doc.total - 4443.52).abs() < 1e-9);
    assert_eq!(
        doc.comments,
        "Late afternoon is best. Backup contact is Nancy Billsmer @ 338-4338."
    );

    // round trip
    let yaml2 = serde_saphyr::to_string(&doc)?;
    let doc2 = serde_saphyr::from_str(&yaml2)?;

    assert_eq!(doc, doc2, "Round trip");

    Ok(())
}
