import { render } from "preact";
import { ALL_CARDS } from "./card-info";
import App from "./components/app";

async function start() {
    await ALL_CARDS.loadCards();

    render(<App />, document.body);
}

start();
