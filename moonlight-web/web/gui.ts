
export interface Component {
    mount(parent: Element): void

    unmount(parent: Element): void
}

export class ComponentHost<T extends Component> {
    private root: Element
    private component: T

    constructor(root: Element, component: T) {
        this.root = root
        this.component = component

        this.component.mount(root)
    }

    destroy() {
        this.component.unmount(this.root)
    }

    getRoot(): Element {
        return this.root
    }
    getComponent(): T {
        return this.component
    }
}

export interface Modal<Output> extends Component {
    onFinish(): Promise<Output>
}

let modalUsed = false
let modalBackground = document.getElementById("modal-overlay")
let modalParent = document.getElementById("modal-parent")

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

    modal.mount(modalParent)
    modalBackground?.classList.remove("modal-disabled")

    const output = await modal.onFinish()

    modalBackground?.classList.add("modal-disabled")
    modal.unmount(modalParent)

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

            this.cancelButton.addEventListener("click", () => {
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

export async function showMessage(message: string) {
    const modal = new MessageModal(message)

    await showModal(modal)
}

class MessageModal implements Component, Modal<void> {

    private textElement: HTMLElement = document.createElement("p")
    private okButton: HTMLButtonElement = document.createElement("button")

    constructor(message: string) {
        this.textElement.innerText = message

        this.okButton.innerText = "Ok"
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
            this.okButton.addEventListener("click", () => resolve(), { once: true })
        })
    }
}


export function showErrorPopup(error: string, fatal: boolean = false) {
    if (fatal) {
        alert(error)
    } else {
        alert(error)
    }
}

export type ListComponentInit = {}

export class ListComponent<T extends Component> implements Component {

    private list: Array<T>
    private listElement: HTMLLIElement

    constructor(list?: Array<T>, init?: ListComponentInit) {
        this.list = list ?? []
        if (list) {
            this.internalMountFrom(0)
        }

        this.listElement = document.createElement("li")
    }

    private internalUnmountUntil(index: number) {
        for (let i = this.list.length - 1; i >= index; i--) {
            const element = this.list[i]
            element.unmount(this.listElement)
        }
    }
    private internalMountFrom(index: number) {
        for (let i = index; i < this.list.length; i++) {
            const element = this.list[i]
            element.mount(this.listElement)
        }
    }

    insert(index: number, value: T) {
        if (index == this.list.length) {
            this.list.push(value)
            value.mount(this.listElement)
        } else {
            this.internalUnmountUntil(index)

            this.list.splice(index, 0, value)

            this.internalUnmountUntil(index)
        }
    }
    remove(index: number) {
        if (index == this.list.length - 1) {
            const element = this.list.pop()
            if (element) {
                element.mount(this.listElement)
            }
        } else {
            this.internalUnmountUntil(index)

            this.list.splice(index, 1)

            this.internalUnmountUntil(index)
        }
    }

    append(value: T) {
        this.insert(this.getList().length, value)
    }
    removeValue(value: T) {
        const index = this.list.indexOf(value)
        if (index != -1) {
            this.remove(index)
        }
    }


    getList(): readonly T[] {
        return this.list
    }

    mount(parent: Element): void {
        parent.appendChild(this.listElement)
    }
    unmount(parent: Element): void {
        parent.appendChild(this.listElement)
    }
}