import { decode } from "deckstrings";
import { ALL_CARDS, CardInfo } from "./card-info";

export type SlotPair = [CardInfo, DeckSlot];

export type DeckSlot =  {
    currentAmount: number,
    maximumAmount: number,
};

export class DeckInfo {
    private contents: Map<CardInfo, DeckSlot>;

    static validate(deckstring: string) {
        try {
            return decode(deckstring).cards;
        } catch {
            return null;
        }
    }

    constructor(deck?: string | Map<CardInfo, DeckSlot>) {
        if (typeof deck == "string") {
            this.contents = new Map();

            for (let [id, count] of (DeckInfo.validate(deck) || [])) {
                this.contents.set(
                    ALL_CARDS.get(id)!,
                    { currentAmount: count, maximumAmount: count }
                );
            }
        } else {
            this.contents = deck || new Map();
        }
    }

    get(card: CardInfo): DeckSlot | undefined {
        let entry = this.contents.get(card);

        return entry ? { ...entry } : undefined;
    }

    set(card: CardInfo, info: DeckSlot) {
        this.contents.set(card, { ...info });
    }

    reset() {
        for (let [card, info] of this) {
            this.set(card, { ...info, currentAmount: info.maximumAmount });
        }
    }

    delete(card: CardInfo) {
        this.contents.delete(card);
    }

    clone() {
        function* cloneEntries(map: Map<CardInfo, DeckSlot>): Generator<[CardInfo, DeckSlot]> {
            for (let [card, { ...info }] of map) {
                yield [card, info];
            }
        }

        let contents = new Map(cloneEntries(this.contents));

        return new DeckInfo(contents);
    }

    keys() {
        return this.contents.keys();
    }

    values() {
        return this.contents.values();
    }

    [Symbol.iterator]() {
        return this.contents.entries();
    }
}
