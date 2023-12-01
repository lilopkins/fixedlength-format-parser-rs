# Fixed-length Format Parser

Write parsers for fixed-length formatted flat files quickly.

## Goal

This project provides a macro to quickly build a parser for formats where there exists records which begin with an identifier, then continue with a format of fixed length.

## Example

Consider the following file:

```text
AA20231201
PNDarth     Vader               123 Death Star Way            AB12 3CD
ZZ001
```

### Schema

This could be seen as a file with the format specified below:

Record types:

- Header ("AA")
- Person ("PN")
- Trailer ("ZZ")

> **Note:** Record types can be represented by any number of characters, but must always be present at the start of the record, and must always be the same length as each other.

**Header**:

Field | Value | Length | Description
------|-------|--------|------------
Type | "AA" | 2 | The header type
Date | YYYYMMDD | 8 | The date the file was produced.

**Person**:

Field | Value | Length | Description
------|-------|--------|------------
Type | "PN" | 2 | The person type
Forename | String | 10 | The forename of the person
Surname | String | 20 | The surname of the person
Address line | String | 30 | The address line
Postcode | String | 8 | The postcode in UK format

**Trailer**:

Field | Value | Length | Description
------|-------|--------|------------
Type | "ZZ" | 2 | The trailer type
Number of records | Number | 3 | The number of records in the file.

### Example Parser

```rust
use fixedlength_format_parser::FixedLengthFormatParser;

#[derive(FixedLengthFormatParser)]
pub enum PersonRecord {
    #[record_type = "AA"]
    Header {
        #[field_starts = 2]
        #[field_length = 8]
        // You could also specify the end instead of the length. End is exclusive.
        // #[field_ends = 10]
        date: String,
    },

    #[record_type = "PN"]
    Person {
        #[field_starts = 2]
        #[field_length = 10]
        forename: String,

        // `field_starts` is optional. If unspecified, it starts at 0 then increments by the length for each field.
        #[field_length = 20]
        surname: String,

        #[field_length = 30]
        address_line: String,

        #[field_length = 8]
        postcode: String,
    },

    #[record_type = "ZZ"]
    Trailer {
        // Any type is allowed, as long as it implements [`std::str::FromStr`].
        #[field_starts = 2]
        #[field_length = 3]
        num_records: usize,
    },
}

// You can now invoke the parser for each record:
fn parse_record(record: &str) -> PersonRecord {
    record.parse::<PersonRecord>().expect("the record should be valid")
}
```
