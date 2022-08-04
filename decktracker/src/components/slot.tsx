import { Component } from "preact";
import { useState } from "preact/hooks";
import { CardInfo } from "../card-info";

type Props = {
    card: CardInfo,
    amountRemaining: number,
    onClick: (card: CardInfo) => void
};

export default function Slot(props: Props) {
    let displayedCount = props.card.rarity === "LEGENDARY"
        ? "★"
        : ([0, 1].includes(props.amountRemaining)
            ? null
            : props.amountRemaining.toString());

    let cardCount = displayedCount !== null && (
        <div class="section card-count">
            {displayedCount}
        </div>
    );

    return (
        <div class="deck-slot" onClick={() => props.onClick(props.card)}>
            <div class={`section mana-cost rarity-${props.card.rarity.toLowerCase()}`}>
                {props.card.cost}
            </div>
            <div class="section card-name" style={`--tile-url: url(${props.card.thumbnailUrl()});`}>
                {props.card.name}
            </div>
            {cardCount}
        </div>
    );
}
