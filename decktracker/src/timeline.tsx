import { decode } from "deckstrings";
import { ALL_CARDS, CardInfo } from "./card-info";

interface Action<S> {
    execute(state: S): S;
    undo(state: S): S;
}

type UpdateArgs<S> = {
    by: number,
    when: boolean,
    action: keyof Action<S>,
    errorMessage: string,
};

export class Timeline<S> {
    private actions: Action<S>[] = [];
    private cursor: number = 0;
    private state: S;

    constructor(state: S) {
        this.state = state;
    }

    get isCurrent(): boolean {
        return this.cursor === this.actions.length;
    }

    get canUndo(): boolean {
        return this.cursor !== 0;
    }

    get canRedo(): boolean {
        return this.isCurrent;
    }

    // TODO: value type

    private update({by, when, action, errorMessage}: UpdateArgs<S>) {
        if (!when) {
            throw new Error(errorMessage);
        }

        // Not compound assignment so that this method is atomic. i.e, if a method call throws, no changes are made.
        let newCursor = this.cursor + by;
        this.state = this.actions[newCursor][action](this.state);
        this.cursor = newCursor;
    }

    execute(action: Action<S>) {
        if (!this.isCurrent) {
            this.actions.splice(this.cursor);
        }

        this.state = action.execute(this.state);
        this.actions.push(action);
        this.cursor += 1;
    }

    undo() {
        this.update({
            by: -1,
            when: this.canUndo,
            action: "undo",
            errorMessage: "Cannot undo any further",
        });
    }

    redo() {
        this.update({
            by: 1,
            when: this.canRedo,
            action: "execute",
            errorMessage: "Nothing to redo",
        });
    }
}
