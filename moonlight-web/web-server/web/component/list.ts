import { Component } from "./index.js"

export type ListComponentInit = {
    listClasses?: string[],
    elementLiClasses?: string[]
    remountIsInsert?: boolean
}

export class ListComponent<T extends Component> implements Component {

    private list: Array<T>

    private mounted: number = 0
    private remountIsInsertTransition: boolean

    private listElement = document.createElement("ul")
    private liElements: Array<HTMLLIElement> = []
    private liClasses: string[]

    constructor(list?: Array<T>, init?: ListComponentInit) {
        this.list = list ?? []
        if (list) {
            this.internalMountFrom(0)
        }

        this.listElement.classList.add("list-like")
        if (init?.listClasses) {
            this.listElement.classList.add(...init?.listClasses)
        }
        this.liClasses = init?.elementLiClasses ?? []

        this.remountIsInsertTransition = init?.remountIsInsert ?? true
    }

    private elementAt(index: number): HTMLLIElement {
        let li = this.liElements[index]
        if (!li) {
            li = document.createElement("li")
            li.classList.add(...this.liClasses)

            this.liElements[index] = li
        }

        return li
    }

    private onAnimElementInserted(index: number) {
        const element = this.liElements[index]

        // let the element render and then add "list-show" for transitions :)
        setTimeout(() => {
            element.classList.add("list-show")
        }, 0)
    }
    private onAnimElementRemoved(index: number) {
        let element
        while ((element = this.liElements[index]).classList.contains("list-show")) {
            element.classList.remove("list-show")
        }
    }

    private internalUnmountUntil(index: number) {
        for (let i = this.list.length - 1; i >= index; i--) {
            const liElement = this.elementAt(i)
            this.listElement.removeChild(liElement)

            const element = this.list[i]
            element.unmount(liElement)
        }
    }
    private internalMountFrom(index: number) {
        if (this.mounted <= 0) {
            return;
        }

        for (let i = index; i < this.list.length; i++) {
            let liElement = this.elementAt(i)
            this.listElement.appendChild(liElement)

            const element = this.list[i]
            element.mount(liElement)
        }
    }

    insert(index: number, value: T) {
        if (index == this.list.length) {
            const liElement = this.elementAt(index)

            this.list.push(value)

            value.mount(liElement)
            this.listElement.appendChild(liElement)
        } else {
            this.internalUnmountUntil(index)

            this.list.splice(index, 0, value)

            this.internalMountFrom(index)
        }

        this.onAnimElementInserted(index)
    }
    remove(index: number): T | null {
        if (index == this.list.length - 1) {
            const element = this.list.pop()
            const liElement = this.liElements[index]

            if (element && liElement) {
                element.unmount(liElement)

                this.listElement.removeChild(liElement)
                return element
            }
        } else {
            this.internalUnmountUntil(index)

            const element = this.list.splice(index, 1)

            this.internalMountFrom(index)

            return element[0] ?? null
        }

        this.onAnimElementRemoved(this.list.length + 1)

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
        this.mounted++

        parent.appendChild(this.listElement)

        // Mount all elements
        if (this.mounted == 1) {
            this.internalMountFrom(0)

            if (this.remountIsInsertTransition) {
                for (let i = 0; i < this.list.length; i++) {
                    this.onAnimElementInserted(i)
                }
            }
        }
    }
    unmount(parent: Element): void {
        this.mounted--

        parent.removeChild(this.listElement)

        // Unmount all elements
        if (this.mounted == 0) {
            this.internalUnmountUntil(0)

            if (this.remountIsInsertTransition) {
                for (let i = 0; i < this.list.length; i++) {
                    this.onAnimElementRemoved(i)
                }
            }
        }
    }
}
