import { Component } from "./component.js"
import { showErrorPopup } from "./error.js"

export interface Modal<Output> extends Component {
    onFinish(): Promise<Output>
}

let modalUsed = false
let modalBackground = document.getElementById("modal-overlay")
let modalParent = document.getElementById("modal-parent")
let previousModal: Modal<unknown> | null = null

export async function showModal<Output>(modal: Modal<Output>): Promise<Output | null> {
    if (modalParent == null) {
        showErrorPopup("cannot find modal parent")
        return null
    }
    if (modalBackground == null) {
        showErrorPopup("the modal overlay cannot be found")
    }

    if (modalUsed) {
        showErrorPopup("cannot mount 2 modals at the same time")
        return null
    }

    if (previousModal) {
        previousModal.unmount(modalParent)
    }
    previousModal = modal

    modalUsed = true
    modal.mount(modalParent)
    modalBackground?.classList.remove("modal-disabled")

    const output = await modal.onFinish()

    modalBackground?.classList.add("modal-disabled")
    modalUsed = false

    return output
}

export abstract class FormModal<Output> implements Component, Modal<Output | null> {

    private formElement: HTMLFormElement = document.createElement("form")
    private mounted: boolean = false
    private submitButton: HTMLButtonElement = document.createElement("button")
    private cancelButton: HTMLButtonElement = document.createElement("button")

    constructor() {
        this.submitButton.type = "submit"
        this.submitButton.innerText = "Ok"

        this.cancelButton.innerText = "Cancel"
    }

    abstract reset(): void
    abstract submit(): Output | null

    abstract mountForm(form: HTMLFormElement): void

    mount(parent: Element): void {
        if (!this.mounted) {
            this.mountForm(this.formElement)
            this.formElement.appendChild(this.submitButton)
            this.formElement.appendChild(this.cancelButton)
        }

        this.reset()

        parent.appendChild(this.formElement)
    }
    unmount(parent: Element): void {
        parent.removeChild(this.formElement)
    }

    onFinish(): Promise<Output | null> {
        const abortController = new AbortController()

        return new Promise((resolve, reject) => {
            this.formElement.addEventListener("submit", event => {
                event.preventDefault()

                const output = this.submit()

                if (output == null) {
                    return
                }

                abortController.abort()
                resolve(output)
            }, { signal: abortController.signal })

            this.cancelButton.addEventListener("click", event => {
                event.preventDefault()

                abortController.abort()
                resolve(null)
            }, { signal: abortController.signal })
        })
    }
}

export async function showPrompt(prompt: string, promptInit?: PromptInit): Promise<string | null> {
    const modal = new PromptModal(prompt, promptInit)

    return await showModal(modal)
}

type PromptInit = {
    defaultValue?: string,
    name?: string,
    type?: "text" | "password",
}

class PromptModal extends FormModal<string> {
    private message: HTMLElement = document.createElement("p")
    private textInput: HTMLInputElement = document.createElement("input")

    constructor(prompt: string, init?: PromptInit) {
        super()

        this.message.innerText = prompt

        if (init?.type) {
            this.textInput.type = init?.type
        }
        if (init?.defaultValue) {
            this.textInput.defaultValue = init?.defaultValue
        }
        if (init?.name) {
            this.textInput.name = init?.name
        }
    }

    reset(): void {
        this.textInput.value = ""
    }
    submit(): string | null {
        return this.textInput.value
    }

    mountForm(form: HTMLFormElement): void {
        form.appendChild(this.message)
        form.appendChild(this.textInput)
    }
}

type MessageInit = {
    signal?: AbortSignal
}

export async function showMessage(message: string, init?: MessageInit) {
    const modal = new MessageModal(message, init)

    await showModal(modal)
}

class MessageModal implements Component, Modal<void> {

    private signal?: AbortSignal
    private textElement: HTMLElement = document.createElement("p")
    private okButton: HTMLButtonElement = document.createElement("button")

    constructor(message: string, init?: MessageInit) {
        this.textElement.innerText = message

        this.okButton.innerText = "Ok"

        this.signal = init?.signal
    }

    mount(parent: Element): void {
        parent.appendChild(this.textElement)
        parent.appendChild(this.okButton)
    }
    unmount(parent: Element): void {
        parent.removeChild(this.textElement)
        parent.removeChild(this.okButton)
    }

    onFinish(): Promise<void> {
        return new Promise((resolve, reject) => {
            let customController: AbortController | null = null
            if (this.signal) {
                customController = new AbortController()
                this.signal.addEventListener("abort", () => {
                    resolve()
                    customController?.abort()
                }, { once: true, signal: customController.signal })
            }

            this.okButton.addEventListener("click", () => {
                resolve()
                customController?.abort()
            }, { signal: customController?.signal })
        })
    }
}

