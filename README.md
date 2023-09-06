# NiceHTML

NiceHTML is a language designed for building web pages, very similar to html, but with a cleaner and more intuitive syntax.
It also has support for variables and simple functions.

The transpiler is written in rust and called using wasm to transpile nicehtml code to html and insert it into the DOM.


## Usage

Add this line to your HTML file:

```html
<script type="module" src="https://cdn.jsdelivr.net/gh/JonasLoos/NiceHTML@built/nicehtml.js"></script>
```

Then you can write NiceHTML inside script tags with a `type="text/nicehtml"` attribute:

```html
<script type="text/nicehtml">...</script>
```

You can look at [index.html](index.html) for an example.

### Using a fixed version

If you want to do more than just testing NiceHTML, it's probably better to fix the version you are using. E.g.:

```html
<script type="module" src="https://cdn.jsdelivr.net/gh/JonasLoos/NiceHTML@v0.1.1/nicehtml.js"></script>
```


## Example

A very simple NiceHTML example:

```
div .section
    h1 .title
        "NiceHTML"
    p
        "like HTML but nicer!"
```

transpiles to:

```html
<div class="section">
    <h1 class="title">
        NiceHTML
    </h1>
    <p>
        like HTML but nicer!
    </p>
```

A more complex example, using functions:

```
card(title, content) =
    # this creates a card with title and content
    div .card
        h3 .card-header
            $title
        div .card-content
            $content

div .section
    $card("Card 1", "This is the content of card 1")
    $card("Card 2", "This is the content of card 2")
```

More examples can be found in the [examples](examples) folder.


## Build locally

build transpiler:

```bash
cd nicehtml_transpiler
wasm-pack build --target web --dev
```

serve locally, e.g. using python:

```bash
python -m http.server
```
