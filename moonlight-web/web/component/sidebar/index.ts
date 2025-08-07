import { Component } from "../index.js"
import { showErrorPopup } from "../error.js"

export interface Sidebar extends Component {
    extended(): void
    unextend(): void
}

let sidebarMounted = false
let sidebarExtended = false
const sidebarBackground = document.getElementById("sidebar-background")
const sidebarParent = document.getElementById("sidebar-parent")
const sidebarButton = document.getElementById("sidebar-button")
sidebarButton?.addEventListener("click", toggleSidebar)
let sidebarComponent: Sidebar | null = null

export type SidebarEdge = "up" | "down" | "left" | "right"
export type SidebarStyle = {
    edge?: SidebarEdge
    alwaysActive?: boolean
}

export function setSidebarStyle(style: SidebarStyle) {
    const edge = style.edge ?? "left"
    const alwaysActive = style.alwaysActive ?? false

    // TODO
}

export function toggleSidebar() {
    setSidebarExtended(!isSidebarExtended())
}
export function setSidebarExtended(extended: boolean) {
    if (extended == sidebarExtended) {
        return
    }

    if (extended) {
        sidebarBackground?.classList.add("sidebar-show")
    } else {
        sidebarBackground?.classList.remove("sidebar-show")
    }
    sidebarExtended = extended
}
export function isSidebarExtended(): boolean {
    return sidebarExtended
}

export function setSidebar(sidebar: Sidebar | null) {
    if (sidebarParent == null) {
        showErrorPopup("failed to get sidebar")
        return
    }

    if (sidebarMounted) {
        // unmount
        sidebarComponent?.unmount(sidebarParent)
        sidebarMounted = false
    }
    if (sidebar) {
        // mount
        sidebarComponent = sidebar
        sidebar?.mount(sidebarParent)
    }
}
