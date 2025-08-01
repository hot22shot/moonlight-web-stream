import { DetailedHost, GetDetailedHostResponse, GetHostsResponse, UndetailedHost } from "./api_bindings.js";
import { showErrorPopup } from "./gui/error.js";
import { showMessage, showPrompt } from "./gui/modal.js";

export const ASSETS = {
    HOST_IMAGE: "/resources/desktop_windows-48px.svg",
    HOST_OVERLAY_NONE: "",
    HOST_OVERLAY_LOCK: "/resources/baseline-lock-24px.svg",
    WARN_IMAGE: "/resources/baseline-warning-24px.svg",
    ERROR_IMAGE: "/resources/baseline-error_outline-24px.svg",
}

let currentApi: Api | null = null

export async function getApi(host_url?: string): Promise<Api> {
    if (currentApi) {
        return currentApi
    }

    if (!host_url) {
        host_url = `${window.location.origin}/api`
    }

    let credentials = window.sessionStorage.getItem("credentials");

    while (credentials == null) {
        const testCredentials = await showPrompt("enter credentials", { name: "api-credentials", type: "password" })

        if (!testCredentials) {
            continue;
        }

        let api = { host_url, credentials: testCredentials }

        if (await authenticate(api)) {
            window.sessionStorage.setItem("credentials", testCredentials)

            credentials = api.credentials;

            break;
        } else {
            await showMessage("credentials are not valid")
        }
    }

    currentApi = { host_url, credentials }

    return currentApi
}

export type Api = {
    host_url: string
    credentials: string,
}

export type ApiFetchInit = {
    data?: any,
    parseResponse?: boolean,
}

export async function fetchApi(api: Api, endpoint: string, method: string = "get", init?: ApiFetchInit): Promise<any | null> {
    const response = await fetch(`${api.host_url}/${endpoint}`, {
        method: method,
        headers: {
            "Authorization": `Bearer ${api.credentials}`
        },
        body: init?.data && JSON.parse(init.data),
    })

    if (!response.ok) {
        return null
    }

    if (init?.parseResponse == undefined || init.parseResponse) {
        const json = await response.json()

        return json
    } else {
        return await response.text()
    }
}

export async function authenticate(api: Api): Promise<boolean> {
    const response = await fetchApi(api, "authenticate", "get", { parseResponse: false })

    return response != null
}

export async function getHosts(api: Api): Promise<Array<UndetailedHost>> {
    const response = await fetchApi(api, "hosts", "get")

    if (response == null) {
        showErrorPopup("failed to fetch hosts")
        return []
    }

    return (response as GetHostsResponse).hosts
}
export async function getDetailedHost(api: Api, hostId: number): Promise<DetailedHost | null> {
    const response = await fetchApi(api, `host?host_id=${hostId}`, "get")

    if (response == null) {
        return null
    }

    return (response as GetDetailedHostResponse).host
}