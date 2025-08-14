import { Component, ComponentEvent } from "./index.js"

export type InputInit = {
    defaultValue?: string
    value?: string
    step?: string
    accept?: string
}

export type InputChangeListener = (event: ComponentEvent<InputComponent>) => void

export class InputComponent implements Component {

    private div: HTMLDivElement = document.createElement("div")
    private label: HTMLLabelElement = document.createElement("label")
    private input: HTMLInputElement = document.createElement("input")

    constructor(internalName: string, type: string, displayName?: string, init?: InputInit) {
        if (displayName) {
            this.label.htmlFor = internalName
            this.label.innerText = displayName
            this.div.appendChild(this.label)
        }

        this.input.id = internalName
        this.input.type = type
        if (init?.defaultValue != null) {
            this.input.defaultValue = init.defaultValue
        }
        if (init?.value != null) {
            this.input.value = init.value
        }
        if (init && init.step != null) {
            this.input.step = init.step
        }
        if (init && init.accept != null) {
            this.input.accept = init.accept
        }
        this.div.appendChild(this.input)

        this.input.addEventListener("change", () => {
            this.div.dispatchEvent(new ComponentEvent("ml-change", this))
        })
    }

    reset() {
        this.input.value = ""
    }

    getValue(): string {
        return this.input.value
    }

    getFiles(): FileList | null {
        return this.input.files
    }

    addChangeListener(listener: InputChangeListener, options?: AddEventListenerOptions) {
        this.div.addEventListener("ml-change", listener as any, options)
    }
    removeChangeListener(listener: InputChangeListener) {
        this.div.removeEventListener("ml-change", listener as any)
    }

    mount(parent: HTMLElement): void {
        parent.appendChild(this.div)
    }
    unmount(parent: HTMLElement): void {
        parent.removeChild(this.div)
    }
}