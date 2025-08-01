import { UndetailedHost } from "./api_bindings.js";
import { ASSETS, getApi, getHosts } from "./common.js";
import { Component, ComponentHost, ListComponent, showErrorPopup } from "./gui.js";

// TODO: error handler with popup

async function startApp() {
    const api = await getApi()

    const rootElement = document.getElementById("root");
    if (rootElement == null) {
        showErrorPopup("couldn't find root element", true)
        return;
    }


    const hosts = await getHosts(api)

    const list = new ListComponent()

    hosts.forEach(host => {
        list.append(new Host(host.host_id, host))
    })

    const root = new ComponentHost(rootElement, list)
}

console.log("starting app")
startApp()

class Host implements Component {
    private hostId: number
    private host?: UndetailedHost

    private imageElement: HTMLImageElement = document.createElement("img")
    private nameElement: HTMLElement = document.createElement("p")

    constructor(hostId: number, host?: UndetailedHost) {
        this.hostId = hostId
        this.host = host

        this.imageElement.src = ASSETS.DEFAULT_HOST_IMAGE

        this.updateDisplay()
    }

    updateDisplay() {
        this.nameElement.innerText = this.host?.name ?? "! Unknown !"

        // TODO: update image
    }

    mount(parent: Element): void {
        parent.appendChild(this.imageElement)
        parent.appendChild(this.nameElement)
    }
    unmount(parent: Element): void {
        parent.removeChild(this.imageElement)
        parent.removeChild(this.nameElement)
    }
}