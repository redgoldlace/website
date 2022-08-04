export type Type = "HERO" | "MINION" | "SPELL" | "ENCHANTMENT" | "WEAPON" | "HERO_POWER";
export type Rarity = "COMMON" | "RARE" | "EPIC" | "LEGENDARY";
export type DbfId = number;
export type IdString = string;

export const RARITY_LEVEL = {
    "COMMON": 1,
    "RARE": 2,
    "EPIC": 3,
    "LEGENDARY": 4,
};

type Payload = {[_: string]: any};

function check<T>(value: T | null | undefined, defaultValue: T | undefined = undefined): T {
    if (value != null) {
        return value;
    }
    
    if (defaultValue != null) {
        return defaultValue;
    }

    throw new Error("Missing value in payload!");
}

export class CardInfo {
    readonly id: IdString;
    readonly dbfId: DbfId;
    readonly name: string;
    readonly cost: number;
    readonly rarity: Rarity;
    readonly type: Type;

    constructor(payload: Payload) {
        this.id = check(payload["id"]);
        this.dbfId = check(payload["dbfId"]);
        this.name = check(payload["name"]);
        this.cost = check(payload["cost"], 0);
        this.rarity = payload["rarity"] === "FREE" ? "COMMON" : check(payload["rarity"]);
        this.type = check(payload["type"]);
    }

    thumbnailUrl(): string {
        return `https://art.hearthstonejson.com/v1/tiles/${this.id}.webp`
    }
};

class CardSet {
    static readonly ALL_CARDS = new CardSet();
    private cards: Map<DbfId, CardInfo>;

    private constructor() {
        this.cards = new Map();
    }

    public get(dbfId: DbfId): CardInfo | undefined {
        return this.cards.get(dbfId);
    }

    public async loadCards() {
        let response = await fetch("https://api.hearthstonejson.com/v1/latest/enUS/cards.collectible.json");

        if (!response.ok) {
            throw new Error("Unable to load cards!");
        }

        function* unpackJson(response: Payload[]): Generator<[number, CardInfo]> {
            for (let payload of response) {
                let card = new CardInfo(payload);

                yield [card.dbfId, card];
            }
        }

        let cards = unpackJson(await response.json());

        this.cards = new Map(cards);
    }
}

export const ALL_CARDS = CardSet.ALL_CARDS;
