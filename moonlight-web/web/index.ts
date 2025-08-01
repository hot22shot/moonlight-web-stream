import { Api, authenticate } from "./api.js"

// TODO: error handler with popup

async function getApi(host_url?: string): Promise<Api> {
    if (!host_url) {
        host_url = `${window.location.origin}/api`;
    }

    let credentials = window.sessionStorage.getItem("credentials");

    if (credentials != null) {
        return { host_url, credentials };
    }

    credentials = window.prompt("credentials")
    if (credentials == null) {
        const error = "please reload and enter valid credentials";
        alert(error);

        throw error
    }

    const api = { host_url, credentials };

    authenticate(api)

    window.sessionStorage.setItem("credentials", credentials);

    return api;
}

async function startApp() {
    const api: Api = await getApi();

}

console.log("starting app")
startApp()