group "default" {
    targets = ["website"]
}

target "website" {
    contexts = {
        website = "website"
        website_content = "website/content"
        decktracker = "decktracker"
    }

    tags = ["kaylynn234/kaylynn.gay:latest"]
}