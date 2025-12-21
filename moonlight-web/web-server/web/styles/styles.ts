import { defaultStreamSettings, getLocalStreamSettings } from "../component/settings_menu.js"

export type PageStyle = "standard" | "old"

let currentStyle: null | PageStyle = null

let styleLink = document.createElement("link")
styleLink.rel = "stylesheet"

export function setStyle(style: PageStyle) {
    if (!currentStyle) {
        document.head.appendChild(styleLink)
    }

    currentStyle = style

    styleLink.href = `styles/${style}.css`
}
export function getStyle(): PageStyle {
    // Style is set at the bottom of this page so it cannot be null
    return currentStyle as PageStyle
}

const settings = getLocalStreamSettings()
const defaultSettings = defaultStreamSettings()

setStyle(settings?.pageStyle ?? defaultSettings.pageStyle)