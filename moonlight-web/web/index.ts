import { Api, getApi, putHost } from "./common.js";
import { Component, ComponentHost } from "./gui/component.js";
import { showErrorPopup } from "./gui/error.js";
import { showModal } from "./gui/modal.js";
import { AddHostModal, HostList } from "./host.js";

// TODO: error handler with popup

async function startApp() {
    const api = await getApi()

    const rootElement = document.getElementById("root");
    if (rootElement == null) {
        showErrorPopup("couldn't find root element", true)
        return;
    }

    const rootComponent = new MainApp(api)
    const root = new ComponentHost(rootElement, rootComponent)

    rootComponent.forceFetch()
}

console.log("starting app")
startApp()

class MainApp implements Component {
    private api: Api

    private moonlightTextElement = document.createElement("h1")
    private hostAddButton: HTMLButtonElement = document.createElement("button")
    private hostList: HostList

    constructor(api: Api) {
        this.api = api

        // Moonlight text
        this.moonlightTextElement.innerHTML = "Moonlight Web"

        // Host add button
        this.hostAddButton.innerText = "Add Host"
        this.hostAddButton.addEventListener("click", this.addHost.bind(this))

        // Host list
        this.hostList = new HostList(api)
    }

    private async addHost() {
        const modal = new AddHostModal()

        let host = await showModal(modal);
        if (host) {
            const newHost = await putHost(this.api, host)

            if (newHost) {
                this.hostList.insertHost(newHost)
            } else {
                showErrorPopup("couldn't add host")
            }
        } else {
            showErrorPopup("couldn't add host")
        }
    }

    forceFetch() {
        this.hostList.forceFetch()
    }

    mount(parent: HTMLElement): void {
        parent.appendChild(this.moonlightTextElement)
        parent.appendChild(this.hostAddButton)
        this.hostList.mount(parent)
    }
    unmount(parent: HTMLElement): void {
        parent.removeChild(this.moonlightTextElement)
        parent.removeChild(this.hostAddButton)
        this.hostList.unmount(parent)
    }
}