import { Component } from "./index.js"

export type ListComponentInit = {
    listElementClasses?: string[],
    componentDivClasses?: string[]
}

export class ListComponent<T extends Component> implements Component {

    private list: Array<T>
    private listElement: HTMLLIElement = document.createElement("li")
    private divElements: Array<HTMLDivElement> = []
    private divClasses: string[]

    constructor(list?: Array<T>, init?: ListComponentInit) {
        this.list = list ?? []
        if (list) {
            this.internalMountFrom(0)
        }

        if (init?.listElementClasses) {
            this.listElement.classList.add(...init?.listElementClasses)
        }
        this.divClasses = init?.componentDivClasses ?? []
    }

    private divAt(index: number): HTMLDivElement {
        let div = this.divElements[index]
        if (!div) {
            div = document.createElement("div")
            div.classList.add(...this.divClasses)

            this.divElements[index] = div
        }

        return div
    }

    private onElementInserted(index: number) {
        const element = this.divElements[index]

        // let the element render and then add "show" for transitions :)
        setTimeout(() => {
            element.classList.add("show")
        }, 0)
    }

    private internalUnmountUntil(index: number) {
        for (let i = this.list.length - 1; i >= index; i--) {
            const divElement = this.divAt(i)
            this.listElement.removeChild(divElement)

            const element = this.list[i]
            element.unmount(divElement)
        }
    }
    private internalMountFrom(index: number) {
        for (let i = index; i < this.list.length; i++) {
            let divElement = this.divAt(i)
            this.listElement.appendChild(divElement)

            const element = this.list[i]
            element.mount(divElement)
        }
    }

    insert(index: number, value: T) {
        if (index == this.list.length) {
            const divElement = this.divAt(index)

            this.list.push(value)

            value.mount(divElement)
            this.listElement.appendChild(divElement)
        } else {
            this.internalUnmountUntil(index)

            this.list.splice(index, 0, value)

            this.internalMountFrom(index)
        }

        this.onElementInserted(index)
    }
    remove(index: number): T | null {
        if (index == this.list.length - 1) {
            const element = this.list.pop()
            const divElement = this.divElements[index]

            if (element && divElement) {
                element.unmount(divElement)

                this.listElement.removeChild(divElement)
                return element
            }
        } else {
            this.internalUnmountUntil(index)

            const element = this.list.splice(index, 1)

            this.internalMountFrom(index)

            return element[0] ?? null
        }

        return null
    }

    append(value: T) {
        this.insert(this.get().length, value)
    }
    removeValue(value: T) {
        const index = this.get().indexOf(value)
        if (index != -1) {
            this.remove(index)
        }
    }

    clear() {
        this.internalUnmountUntil(0)

        this.list.splice(0, this.list.length)
    }

    get(): readonly T[] {
        return this.list
    }

    mount(parent: Element): void {
        // TODO: mount and unmount here: maybe counter!
        parent.appendChild(this.listElement)
    }
    unmount(parent: Element): void {
        parent.removeChild(this.listElement)
    }
}
