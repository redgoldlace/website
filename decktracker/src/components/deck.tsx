import { decode } from "deckstrings";
import { Component } from "preact";
import { StateUpdater, useState } from "preact/hooks";
import { ALL_CARDS, CardInfo, DbfId, Rarity, RARITY_LEVEL } from "../card-info";
import { DeckInfo, SlotPair } from "../deck-info";
import Slot from "./slot";

type OrderingValue = "lt" | "eq" | "gt";
type ReverseOrdering = { [P in OrderingValue]: OrderingValue };
type ConstructOrdering = [OrderingValue] | [number, number] | [string, string] | [Ordering];

class Ordering {
    private static readonly reverseMap: ReverseOrdering = { lt: "gt", gt: "lt", eq: "eq"}
    private static readonly intMap = { lt: -1, gt: 1, eq: 0 };

    readonly value: OrderingValue;

    constructor(...params: ConstructOrdering) {
        if (params.length == 1) {
            let item = params[0];
            this.value = item instanceof Ordering ? item.value : item;

            return;
        }

        let [a, b] = params;

        this.value = a === b ? "eq" : (a > b ? "gt" : "lt");
    }

    then(...other: ConstructOrdering): Ordering {
        return this.value == "eq" ? new Ordering(...other) : this;
    }

    static chain(first: Ordering, ...items: Ordering[]): Ordering {
        return items.reduce(
            (previous, current) => previous.then(current),
            new Ordering(first)
        );
    }

    reverse(): Ordering {
        return new Ordering(Ordering.reverseMap[this.value]);
    }

    toInt(): number {
        return Ordering.intMap[this.value];
    }
}

type SortStrategyName = "dbfId" | "manaCost";

class SortStrategy {
    static dbfId([cardA]: SlotPair, [cardB]: SlotPair): Ordering {
        return new Ordering(cardA.dbfId, cardB.dbfId);
    }

    static manaCost([cardA]: SlotPair, [cardB]: SlotPair): Ordering {
        return Ordering.chain(
            new Ordering(cardA.cost, cardB.cost),
            new Ordering(RARITY_LEVEL[cardA.rarity], RARITY_LEVEL[cardB.rarity]),
            new Ordering(cardA.name, cardB.name),
        )
    }
}

export default function Deck() {
    let [inputDeckstring, setInputDeckstring] = useState(localStorage.getItem("lastDeck") || "");
    let [deckContents, setDeckContents] = useState(() => new DeckInfo(inputDeckstring));
    let [sortStrategy, setSortStrategy] = useState("manaCost" as SortStrategyName);

    let valid = Boolean(DeckInfo.validate(inputDeckstring));

    function onDeckInput(event: JSX.TargetedEvent<HTMLInputElement, Event>): void {
        setInputDeckstring((event.target as HTMLInputElement).value);
    }

    function onDeckLoad(event: Event) {
        event.preventDefault();

        if (!valid) {
            return;
        }

        localStorage.setItem("lastDeck", inputDeckstring);

        setDeckContents(new DeckInfo(inputDeckstring));
    }

    function onDeckReload(event: Event) {
        event.preventDefault();

        let newDeck = deckContents.clone();
        newDeck.reset();

        setDeckContents(newDeck);
    }

    function onSlotClicked(card: CardInfo) {
        setDeckContents(current => {
            let newDeck = current.clone();

            let slot = newDeck.get(card)!;
            slot.currentAmount = Math.max(0, slot.currentAmount - 1);
            newDeck.set(card, slot);

            return newDeck;
        })
    }

    function* slots() {
        let strategy = SortStrategy[sortStrategy];
        let sortedContents = Array.from(deckContents).sort((a, b) => strategy(a, b).toInt());

        for (let [card, slot] of sortedContents) {
            if (slot.currentAmount === 0) continue;

            yield <Slot card={card} amountRemaining={slot.currentAmount} onClick={onSlotClicked} />;
        }
    }

    let textboxClasses = valid || !inputDeckstring ? "textbox form-plain" : "textbox form-plain invalid";

    let total = 0;
    let minions = 0;
    let spells = 0;

    for (let [card, { currentAmount }] of deckContents) {
        total += currentAmount;
        minions += card.type == "MINION" ? currentAmount : 0;
        spells += card.type == "SPELL" ? currentAmount : 0;
    }

    let draw1 = total ? 100 / total : 0;
    let draw2 = Math.min(draw1 * 2, 100);

    return (
        <div class="deck-container">
            <div class="deck-list">
                <form class="form">
                    <input
                        type="text"
                        class={textboxClasses}
                        value={inputDeckstring}
                        placeholder="Enter deck code..."
                        onInput={onDeckInput}
                    />
                    <button class="button button-load-deck form-ok" onClick={onDeckLoad}>
                        load
                    </button>
                    <button class="button button-reset-deck form-danger" onClick={onDeckReload}>
                        reset
                    </button>
                </form>
                <div class="stats-wrapper">
                    <div class="label form form-plain">
                        <i class="fa-solid fa-wand-magic-sparkles stats-icon"></i>
                        <span>{spells}</span>
                        <span class="faded">|</span>
                        <i class="fa-solid fa-dragon stats-icon"></i>
                        <span>{minions}</span>
                        <span class="faded">|</span>
                        <i class="fa-solid fa-dice stats-icon"></i>
                        <span>{`${draw1.toFixed(1)}%`}</span>
                        <span class="faded">/</span>
                        <span>{`${draw2.toFixed(1)}%`}</span>
                    </div>
                </div>
                <div class="slot-wrapper">
                    {Array.from(slots())}
                </div>
            </div>
        </div>
    );
}
