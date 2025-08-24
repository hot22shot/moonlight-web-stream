import { Component, ComponentEvent } from "./index.js"

export class ElementWithLabel implements Component {
    protected div: HTMLDivElement = document.createElement("div")
    private label: HTMLLabelElement = document.createElement("label")

    constructor(internalName: string, displayName?: string) {
        if (displayName) {
            this.label.htmlFor = internalName
            this.label.innerText = displayName
            this.div.appendChild(this.label)
        }
    }

    mount(parent: HTMLElement): void {
        parent.appendChild(this.div)
    }
    unmount(parent: HTMLElement): void {
        parent.removeChild(this.div)
    }
}

export type InputInit = {
    defaultValue?: string
    value?: string
    checked?: boolean
    step?: string
    accept?: string
    inputMode?: string
}

export type InputChangeListener = (event: ComponentEvent<InputComponent>) => void

export class InputComponent extends ElementWithLabel {

    private input: HTMLInputElement = document.createElement("input")

    constructor(internalName: string, type: string, displayName?: string, init?: InputInit) {
        super(internalName, displayName)

        this.input.id = internalName
        this.input.type = type
        if (init?.defaultValue != null) {
            this.input.defaultValue = init.defaultValue
        }
        if (init?.value != null) {
            this.input.value = init.value
        }
        if (init && init.checked != null) {
            this.input.checked = init.checked
        }
        if (init && init.step != null) {
            this.input.step = init.step
        }
        if (init && init.accept != null) {
            this.input.accept = init.accept
        }
        if (init && init.inputMode != null) {
            this.input.inputMode = init.inputMode
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

    isChecked(): boolean {
        return this.input.checked
    }

    getFiles(): FileList | null {
        return this.input.files
    }

    setEnabled(enabled: boolean) {
        this.input.disabled = !enabled
    }

    addChangeListener(listener: InputChangeListener, options?: AddEventListenerOptions) {
        this.div.addEventListener("ml-change", listener as any, options)
    }
    removeChangeListener(listener: InputChangeListener) {
        this.div.removeEventListener("ml-change", listener as any)
    }
}

export type SelectInit = {
    preSelectedOption?: string
    displayName?: string,
}

export class SelectComponent extends ElementWithLabel {

    private selectElement = document.createElement("select")

    constructor(internalName: string, options: Array<{ value: string, name: string }>, init?: SelectInit) {
        super(internalName, init?.displayName)

        for (const option of options) {
            const optionElement = document.createElement("option")
            optionElement.innerText = option.name
            optionElement.value = option.value

            if (init && init.preSelectedOption == option.value) {
                optionElement.selected = true
            }

            this.selectElement.appendChild(optionElement)
        }

        this.div.appendChild(this.selectElement)

        this.selectElement.addEventListener("change", () => {
            this.div.dispatchEvent(new ComponentEvent("ml-change", this))
        })
    }

    getValue() {
        return this.selectElement.value
    }

    setOptionEnabled(value: string, enabled: boolean) {
        for (const optionElement of this.selectElement.options) {
            if (optionElement.value == value) {
                optionElement.disabled = !enabled
            }
        }
    }

    addChangeListener(listener: InputChangeListener, options?: AddEventListenerOptions) {
        this.div.addEventListener("ml-change", listener as any, options)
    }
    removeChangeListener(listener: InputChangeListener) {
        this.div.removeEventListener("ml-change", listener as any)
    }
}