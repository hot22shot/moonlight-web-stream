import { App, DeleteHostQuery, DetailedHost, GetAppImageQuery, GetAppsQuery, GetAppsResponse, GetHostQuery, GetHostResponse, GetHostsResponse, PostCancelRequest, PostCancelResponse, PostPairRequest, PostPairResponse1, PostPairResponse2, PutHostRequest, PutHostResponse, UndetailedHost } from "./api_bindings.js";
import { showErrorPopup } from "./component/error.js";
import { InputComponent } from "./component/input.js";
import { FormModal } from "./component/modal/form.js";
import { showMessage, showModal } from "./component/modal/index.js";
import { buildUrl } from "./config_.js";

let currentApi: Api | null = null

export async function getApi(host_url?: string): Promise<Api> {
    if (currentApi) {
        return currentApi
    }

    if (!host_url) {
        host_url = buildUrl("/api")
    }

    let credentials = sessionStorage.getItem("mlCredentials");

    while (credentials == null) {
        const prompt = new ApiCredentialsPrompt()
        const testCredentials = await showModal(prompt)

        if (testCredentials == null) {
            continue;
        }

        let api = { host_url, credentials: testCredentials }

        if (await apiAuthenticate(api)) {
            sessionStorage.setItem("mlCredentials", testCredentials)

            credentials = api.credentials;

            break;
        } else {
            await showMessage("Credentials are not Valid")
        }
    }

    currentApi = { host_url, credentials }

    return currentApi
}

class ApiCredentialsPrompt extends FormModal<string> {

    private text: HTMLElement = document.createElement("p")
    private credentials: InputComponent
    private credentialsFile: InputComponent

    constructor() {
        super()

        this.text.innerText = "Enter Credentials"

        this.credentials = new InputComponent("ml-api-credentials", "password", "Credentials")

        this.credentialsFile = new InputComponent("ml-api-credentials-file", "file", "Credentials as File", { accept: ".txt" })
    }

    reset(): void {
        this.credentials.reset()
    }
    submit(): string | null {
        return this.credentials.getValue()
    }

    onFinish(abort: AbortSignal): Promise<string | null> {
        const abortController = new AbortController()
        abort.addEventListener("abort", abortController.abort.bind(abortController))

        return new Promise((resolve, reject) => {
            this.credentialsFile.addChangeListener((event) => {
                const files = this.credentialsFile.getFiles()
                if (files && files.length >= 1) {
                    const file = files[0]

                    file.text().then((credentials) => {
                        abortController.abort()

                        resolve(credentials)
                    })
                }
            }, { signal: abortController.signal })

            super.onFinish(abortController.signal).then((data) => {
                abortController.abort()
                resolve(data)
            }, (data) => {
                abortController.abort()
                reject(data)
            })
        })
    }

    mountForm(form: HTMLFormElement): void {
        form.appendChild(this.text)

        this.credentials.mount(form)
        this.credentialsFile.mount(form)
    }
}

export type Api = {
    host_url: string
    credentials: string,
}

export type ApiFetchInit = {
    json?: any,
    query?: any,
}

export function isDetailedHost(host: UndetailedHost | DetailedHost): host is DetailedHost {
    return (host as DetailedHost).https_port !== undefined
}

function buildRequest(api: Api, endpoint: string, method: string, init?: { response?: "json" | "ignore" } & ApiFetchInit): [string, RequestInit] {
    const query = new URLSearchParams(init?.query)
    const queryString = query.size > 0 ? `?${query.toString()}` : "";
    const url = `${api.host_url}${endpoint}${queryString}`

    const headers: any = {
        "Authorization": `Bearer ${api.credentials}`,
    };

    if (init?.json) {
        headers["Content-Type"] = "application/json";
    }

    const request = {
        method: method,
        headers,
        body: init?.json && JSON.stringify(init.json)
    }

    return [url, request]
}

export async function fetchApi(api: Api, endpoint: string, method: string, init?: { response?: "json" } & ApiFetchInit): Promise<any | null>
export async function fetchApi(api: Api, endpoint: string, method: string, init: { response: "ignore" } & ApiFetchInit): Promise<Response | null>

export async function fetchApi(api: Api, endpoint: string, method: string = "get", init?: { response?: "json" | "ignore" } & ApiFetchInit) {
    const [url, request] = buildRequest(api, endpoint, method, init)

    const response = await fetch(url, request)

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

export async function apiAuthenticate(api: Api): Promise<boolean> {
    const response = await fetchApi(api, "/authenticate", "get", { response: "ignore" })

    return response != null
}

export async function apiGetHosts(api: Api): Promise<Array<UndetailedHost>> {
    const response = await fetchApi(api, "/hosts", "get")

    if (response == null) {
        showErrorPopup("failed to fetch hosts")
        return []
    }

    return (response as GetHostsResponse).hosts
}
export async function apiGetHost(api: Api, query: GetHostQuery): Promise<DetailedHost | null> {
    const response = await fetchApi(api, "/host", "get", { query })

    if (response == null) {
        return null
    }

    return (response as GetHostResponse).host
}
export async function apiPutHost(api: Api, data: PutHostRequest): Promise<DetailedHost | null> {
    const response = await fetchApi(api, "/host", "put", { json: data })

    if (response == null) {
        return null
    }

    return (response as PutHostResponse).host
}
export async function apiDeleteHost(api: Api, query: DeleteHostQuery): Promise<boolean> {
    const response = await fetchApi(api, "/host", "delete", { query, response: "ignore" })

    return response != null
}

export async function apiPostPair(api: Api, request: PostPairRequest): Promise<{ pin: string, result: Promise<DetailedHost | null> } | { error: string } | null> {
    const response = await fetchApi(api, "/pair", "post", {
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

export async function apiGetApps(api: Api, query: GetAppsQuery): Promise<Array<App> | null> {
    const response = await fetchApi(api, "/apps", "get", { query }) as GetAppsResponse

    return response?.apps
}

export async function apiGetAppImage(api: Api, query: GetAppImageQuery): Promise<Blob | null> {
    const response = await fetchApi(api, "/app/image", "get", {
        query,
        response: "ignore"
    })

    if (!response) {
        return null
    }

    const data = await response.blob()

    return data
}

export async function apiHostCancel(api: Api, request: PostCancelRequest): Promise<PostCancelResponse | null> {
    const response = await fetchApi(api, "/host/cancel", "POST", {
        json: request
    })

    if (!response) {
        return null
    }

    return response as PostCancelResponse
}