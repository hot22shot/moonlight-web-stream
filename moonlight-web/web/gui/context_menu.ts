import { Component, ListComponent } from "./component.js"
import { showErrorPopup } from "./error.js"

document.addEventListener("click", () => removeContextMenu())

export type ContextMenuElement = {
    name: string,
    callback(event: MouseEvent): void
}

export type ContextMenuInit = {
    elements?: ContextMenuElement[]
}

const contextMenuElement = document.getElementById("context-menu")
const contextMenuList = new ListComponent<ContextMenuElementComponent>([], {
    listElementClasses: ["context-menu-list"]
})
let contextMenuMounted = false

export function setContextMenu(event: MouseEvent, init?: ContextMenuInit) {
    event.preventDefault()

    if (contextMenuElement == null) {
        showErrorPopup("cannot find the context menu element")
        return;
    }

    contextMenuElement.style.setProperty("left", `${event.pageX}px`)
    contextMenuElement.style.setProperty("top", `${event.pageY}px`)

    contextMenuList.clear()

    for (const element of init?.elements ?? []) {
        contextMenuList.append(new ContextMenuElementComponent(element))
    }

    contextMenuList.mount(contextMenuElement)
    contextMenuElement.classList.remove("context-menu-disabled")

    contextMenuMounted = true
}

export function removeContextMenu() {
    if (contextMenuElement == null) {
        showErrorPopup("cannot find the context menu element")
        return;
    }

    if (contextMenuMounted) {
        contextMenuElement.classList.add("context-menu-disabled")
        contextMenuList.unmount(contextMenuElement)
    }

    contextMenuMounted = false
}

class ContextMenuElementComponent implements Component {
    private nameElement: HTMLElement = document.createElement("p")

    constructor(element: ContextMenuElement) {
        this.nameElement.innerText = element.name

        this.nameElement.classList.add("context-menu-element")
        this.nameElement.addEventListener("click", event => {
            element.callback(event)
        })
    }

    mount(parent: Element): void {
        parent.appendChild(this.nameElement)
    }
    unmount(parent: Element): void {
        parent.removeChild(this.nameElement)
    }
}