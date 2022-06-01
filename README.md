# form-data-builder

This is a simple `multipart/form-data` ([RFC 7578][rfc7578]) document builder.

```rust
use form_data_builder::FormData;

let mut form = FormData::new(Vec::new()); // use a Vec<u8> as a writer
form.content_type_header(); // add this `Content-Type` header to your HTTP request

form.write_path("ferris", "testdata/rustacean-flat-noshadow.png", "image/png")?;
form.write_field("cute", "yes")?;
form.finish(); // returns the writer
```

Looking for a feature-packed, asynchronous, robust, and well-tested `multipart/form-data`
library that validates things like content types? We hope you find one somewhere!

[rfc7578]: https://www.rfc-editor.org/rfc/rfc7578.html

License: MIT-0
