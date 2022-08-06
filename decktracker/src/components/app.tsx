import { Component } from "preact";
import { useEffect, useState } from "preact/hooks";
import { ALL_CARDS } from "../card-info";
import Deck from "./deck";

function Loading() {
    return (
        <div class="loading-container">
            <div class="loading">
                <i class="fa-solid fa-circle-notch loading-icon"></i>
                Fetching card information...
            </div>
        </div>
    );
}

export default function App() {
    let [loaded, setLoaded] = useState(false);

    useEffect(
        // Thank you React very cool
        () => {
            async function load() {
                await ALL_CARDS.loadCards();
                setLoaded(true);
            }

            load();
        },
        []
    );

    return loaded ? <Deck /> : <Loading />;
}
