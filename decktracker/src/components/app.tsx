import { Component } from "preact";
import { useState } from "preact/hooks";
import { ALL_CARDS } from "../card-info";
import Deck from "./deck";

export default function App() {
    let [currentDeck, setCurrentDeck] = useState<string | undefined>();

    return (
        <div class="app">
            <Deck deckstring={currentDeck} />
        </div>
    );
}
