import { DeleteHostQuery, DetailedHost, GetHostQuery, GetHostResponse, GetHostsResponse, PostPairRequest, PostPairResponse1, PostPairResponse2, PutHostRequest, PutHostResponse, UndetailedHost } from "./api_bindings.js";
import { showErrorPopup } from "./gui/error.js";
import { showMessage, showPrompt } from "./gui/modal.js";

export const ASSETS = {
    HOST_IMAGE: "/resources/desktop_windows-48px.svg",
    HOST_OVERLAY_NONE: "",
    HOST_OVERLAY_LOCK: "/resources/baseline-lock-24px.svg",
    WARN_IMAGE: "/resources/baseline-warning-24px.svg",
    ERROR_IMAGE: "/resources/baseline-error_outline-24px.svg",
}

// TODO: move api stuff into api file
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
    response?: "json" | "ignore"
}

export async function fetchApi(api: Api, endpoint: string, method: string, init?: { response?: "json" } & ApiFetchInit): Promise<any | null>
export async function fetchApi(api: Api, endpoint: string, method: string, init: { response: "ignore" } & ApiFetchInit): Promise<Response>

export async function fetchApi(api: Api, endpoint: string, method: string = "get", init?: ApiFetchInit) {
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

    if (init?.response == "ignore") {
        return response
    }

    if (init?.response == undefined || init.response == "json") {
        const json = await response.json()

        return json
    }
}

export async function authenticate(api: Api): Promise<boolean> {
    const response = await fetchApi(api, "authenticate", "get", { response: "ignore" })

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
    const response = await fetchApi(api, "host", "delete", { query, response: "ignore" })

    return response != null
}

export async function postPair(api: Api, request: PostPairRequest): Promise<{ pin: string, result: Promise<DetailedHost | null> } | { error: string } | null> {
    const response = await fetchApi(api, "pair", "post", {
        json: request,
        response: "ignore"
    })
    if (response == null || response.body == null) {
        return null
    }

    const reader = response.body.getReader()
    const decoder = new TextDecoder()

    const read1 = await reader.read();
    const response1 = JSON.parse(decoder.decode(read1.value)) as PostPairResponse1

    if (typeof response1 == "string") {
        return { error: response1 }
    }
    if (read1.done) {
        return { error: "likely InternalServerError" }
    }

    return {
        pin: response1.Pin,
        result: (async () => {
            const read2 = await reader.read();
            const response2 = JSON.parse(decoder.decode(read2.value)) as PostPairResponse2

            if (response2 == "PairError") {
                return null
            } else {
                return response2.Paired
            }
        })()
    }
}