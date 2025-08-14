import CONFIG from "./config.js"

export function buildUrl(path: string): string {
    return `${window.location.origin}${CONFIG?.pathPrefix ?? ""}${path}`
}