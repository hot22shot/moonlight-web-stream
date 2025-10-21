import CONFIG from "./config.js"

export function isUserPasswordAuthenticationEnabled(): boolean {
    return CONFIG?.enable_user_password_authentication ?? true
}

export function buildUrl(path: string): string {
    return `${window.location.origin}${CONFIG?.path_prefix ?? ""}${path}`
}