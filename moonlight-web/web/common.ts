import { GetHosts, UndetailedHost } from "./api_bindings.js";
import { showErrorPopup, showMessage, showPrompt } from "./gui.js";

export const ASSETS = {
    DEFAULT_HOST_IMAGE: "/resources/desktop_windows-48px.svg"
}

export async function getApi(host_url?: string): Promise<Api> {
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

    return { host_url, credentials }
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
    }

    return (response as GetHosts).hosts
}