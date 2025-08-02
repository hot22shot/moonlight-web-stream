import { DeleteHostQuery, DetailedHost, GetHostQuery, GetHostResponse, GetHostsResponse, PutHostRequest, PutHostResponse, UndetailedHost } from "./api_bindings.js";
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
        const testCredentials = await showPrompt("Enter Credentials", { name: "api-credentials", type: "password" })

        if (!testCredentials) {
            continue;
        }

        let api = { host_url, credentials: testCredentials }

        if (await authenticate(api)) {
            window.sessionStorage.setItem("credentials", testCredentials)

            credentials = api.credentials;

            break;
        } else {
            await showMessage("Credentials are not Valid")
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
    json?: any,
    query?: any,
    parseResponse?: boolean,
}

export async function fetchApi(api: Api, endpoint: string, method: string = "get", init?: ApiFetchInit): Promise<any | null> {
    const query = new URLSearchParams(init?.query)
    const queryString = query.size > 0 ? `?${query.toString()}` : "";

    const headers: any = {
        "Authorization": `Bearer ${api.credentials}`,
    };

    if (init?.json) {
        headers["Content-Type"] = "application/json";
    }

    const response = await fetch(`${api.host_url}/${endpoint}${queryString}`, {
        method: method,
        headers,
        body: init?.json && JSON.stringify(init.json)
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
export async function getHost(api: Api, hostId: number): Promise<DetailedHost | null> {
    let query: GetHostQuery = {
        host_id: hostId
    };

    const response = await fetchApi(api, "host", "get", { query })

    if (response == null) {
        return null
    }

    return (response as GetHostResponse).host
}
export async function putHost(api: Api, data: PutHostRequest): Promise<DetailedHost | null> {
    const response = await fetchApi(api, "host", "put", { json: data })

    if (response == null) {
        return null
    }

    return (response as PutHostResponse).host
}
export async function deleteHost(api: Api, query: DeleteHostQuery): Promise<boolean> {
    const response = await fetchApi(api, "host", "delete", { query, parseResponse: false })

    return response != null
}