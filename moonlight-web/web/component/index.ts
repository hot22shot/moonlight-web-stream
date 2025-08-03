
export interface Component {
    mount(parent: HTMLElement): void

    unmount(parent: HTMLElement): void
}

export class ComponentEvent<T extends Component> extends Event {
    component: T

    constructor(type: string, component: T) {
        super(type)

        this.component = component
    }
}

export interface FetchComponent<Data> extends Component {
    forceFetch(): Promise<void>

    updateCache(data: Array<Data>): void
}

export class ComponentHost<T extends Component> {
    private root: HTMLElement
    private component: T

    constructor(root: HTMLElement, component: T) {
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
