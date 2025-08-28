
export type TextEvent = CustomEvent<{ text: string }>

export class ScreenKeyboard {

    private eventTarget = new EventTarget()
    private fakeElement = document.createElement("textarea")

    private visible: boolean = false

    constructor() {
        this.fakeElement.classList.add("hiddeninput")
        this.fakeElement.name = "keyboard"
        this.fakeElement.autocomplete = "off"
        this.fakeElement.autocapitalize = "off"
        this.fakeElement.spellcheck = false
        if ("autocorrect" in this.fakeElement) {
            this.fakeElement.autocorrect = false
        }
        this.fakeElement.addEventListener("input", this.onKeyInput.bind(this))

        // TODO: on blur for visible detection?
    }

    getHiddenElement() {
        return this.fakeElement
    }

    show() {
        if (!this.visible) {
            this.fakeElement.focus()
        }

        this.visible = true
    }
    hide() {
        if (this.visible) {
            this.fakeElement.blur()
        }

        this.visible = false
    }

    isVisible(): boolean {
        return this.visible
    }

    addKeyDownListener(listener: (event: KeyboardEvent) => void) {
        this.eventTarget.addEventListener("keydown", listener as any)
    }
    addKeyUpListener(listener: (event: KeyboardEvent) => void) {
        this.eventTarget.addEventListener("keyup", listener as any)
    }
    addTextListener(listener: (event: TextEvent) => void) {
        this.eventTarget.addEventListener("ml-text", listener as any)
    }

    // -- Events
    private onKeyInput(event: Event) {
        if (!(event instanceof InputEvent)) {
            return
        }
        if (event.isComposing) {
            return
        }

        if ((event.inputType == "insertText" || event.inputType == "insertFromPaste") && event.data != null) {
            const customEvent: TextEvent = new CustomEvent("ml-text", {
                detail: { text: event.data }
            })

            this.eventTarget.dispatchEvent(customEvent)
        } else if (event.inputType == "deleteContentBackward" || event.inputType == "deleteByCut") {
            // these are handled by on key down / up on mobile
        } else if (event.inputType == "deleteContentForward") {
            // these are handled by on key down / up on mobile
        }
    }
}