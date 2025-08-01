
export type Api = {
    host_url: string
    credentials: string,
}

export async function fetch_api(api: Api, endpoint: string, data?: any) {
    fetch(`${api.host_url}/${endpoint}`, {
        headers: {
            "Authorization": `Bearer api.credentials`
        },
        body: data && JSON.parse(data),
    })
}

export async function authenticate(api: Api) {
    fetch_api(api, "authenticate")
}