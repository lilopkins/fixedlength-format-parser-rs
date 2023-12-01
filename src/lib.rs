#![doc = include_str!("../README.md")]

use proc_macro::TokenStream;
use proc_macro2::{Ident, Span};
use quote::{quote, ToTokens, TokenStreamExt};
use syn::{parse_macro_input, Data, DeriveInput, Expr, Lit};

#[derive(Debug)]
struct RecordVariant {
    kind: String,
    enum_name: Ident,
    variant_name: Ident,
    fields: Vec<RecordField>,
}

impl ToTokens for RecordVariant {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let kind = &self.kind;
        let enum_name = &self.enum_name;
        let variant_name = &self.variant_name;
        let fields = &self.fields;
        tokens.append_all(quote! {
            #kind => {
                Ok(#enum_name::#variant_name {
                    #(#fields),*
                })
            }
        });
    }
}

#[derive(Debug)]
struct RecordField {
    /// The name of the field in the enum variant.
    name: Ident,
    /// The point in the line at which this record begins.
    from: usize,
    /// The point in the line at which this record end (exclusive).
    to: usize,
    /// The kind of this record.
    record_kind: String,
    /// The ident of the error enum.
    error_ident: Ident,
}

impl ToTokens for RecordField {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let name = &self.name;
        let from = self.from;
        let to = self.to;
        let record_kind = &self.record_kind;
        let error_ident = &self.error_ident;
        let name_str = name.to_string();
        tokens.append_all(quote! {
            #name: s[#from..#to].parse().map_err(|_| #error_ident::FailedToParse {
                record_type: #record_kind.to_string(),
                field: #name_str.to_string(),
            })?
        });
    }
}

#[proc_macro_derive(
    FixedLengthFormatParser,
    attributes(record_type, field_starts, field_ends, field_length)
)]
pub fn fixed_length_format_parser(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let target_ident = input.ident;
    let error_ident = Ident::new(&format!("{}ParseError", target_ident), Span::call_site());
    let visibility = input.vis;

    let mut record_type_len = 0;
    let mut known_variants = vec![];

    // Validate that all record types are specified
    // Validate that all record types are the same length

    if let Data::Enum(enum_data) = input.data {
        for variant in enum_data.variants {
            assert!(variant.discriminant.is_none(), "Enum variants must not have a discriminant set to be built into a FixedLengthFormatParser.");

            let mut current_cursor = 0;

            for attr in variant.attrs {
                let attr = attr.meta.require_name_value().unwrap();
                if *attr.path.get_ident().unwrap() != "record_type" {
                    panic!("Only the `record_type` attribute is expected on an enum variant.");
                }
                match &attr.value {
                    Expr::Lit(literal) => {
                        match &literal.lit {
                            Lit::Str(st) => {
                                let record_type = st.value();
                                if record_type_len == 0 {
                                    record_type_len = record_type.len();
                                } else if record_type_len != record_type.len() {
                                    panic!("All `record_type`s must be the same length.");
                                }

                                known_variants.push(RecordVariant {
                                    kind: record_type.clone(),
                                    enum_name: target_ident.clone(),
                                    variant_name: variant.ident.clone(),
                                    fields: variant.fields.iter().map(|f| {
                                        let mut from = current_cursor;
                                        let mut length = 0;
                                        let mut to = current_cursor;

                                        for attr in &f.attrs {
                                            let attr = attr.meta.require_name_value().unwrap();
                                            match attr.path.get_ident().unwrap().to_string().as_str() {
                                                "field_starts" => {
                                                    from = get_number(&attr.value);
                                                    to = from + length;
                                                },
                                                "field_ends" => {
                                                    to = get_number(&attr.value);
                                                    length = to - from;
                                                    current_cursor = to;
                                                },
                                                "field_length" => {
                                                    length = get_number(&attr.value);
                                                    to = from + length;
                                                    current_cursor = to;
                                                },
                                                _ => {/* some other ident we don't care about */},
                                            }
                                        }

                                        assert_ne!(length, 0, "`{}` field length is zero!", f.ident.as_ref().unwrap());

                                        RecordField {
                                            name: f.ident.clone().expect("the enum variants must be full structs, not tuples."),
                                            from,
                                            to,
                                            record_kind: record_type.clone(),
                                            error_ident: error_ident.clone(),
                                        }
                                    }).collect(),
                                });
                            },
                            _ => panic!("`record_type` must specify a string literal, e.g.: #[record_type = \"HD\"]"),
                        }
                    },
                    _ => panic!("`record_type` must specify a string literal, e.g.: #[record_type = \"HD\"]"),
                }
            }
        }
    } else {
        panic!("FixedLengthFormatParser can only derive from enums.");
    }

    if record_type_len == 0 {
        panic!("No `record_type`s have been specified, so the parser cannot be built.");
    }

    let expanded = quote! {
        #[derive(Debug)]
        #visibility enum #error_ident {
            InvalidRecordType,
            FailedToParse {
                record_type: String,
                field: String,
            },
        }
        impl ::std::error::Error for #error_ident {}
        impl ::std::fmt::Display for #error_ident {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                match self {
                    Self::InvalidRecordType => write!(f, "invalid record type"),
                    Self::FailedToParse { record_type, field } => write!(f, "failed to parse field `{field}` in {record_type} record."),
                }
            }
        }

        impl ::std::str::FromStr for #target_ident {
            type Err = #error_ident;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let record_type = &s[0..#record_type_len];

                match record_type {
                    #(#known_variants),*
                    _ => Err(#error_ident::InvalidRecordType),
                }
            }
        }
    };

    TokenStream::from(expanded)
}

fn get_number(expr: &Expr) -> usize {
    match &expr {
        Expr::Lit(literal) => match &literal.lit {
            Lit::Int(i) => i
                .base10_parse()
                .expect("expected number for field attribute"),
            _ => panic!("expected number for field attribute"),
        },
        _ => panic!("expected number for field attribute"),
    }
}
