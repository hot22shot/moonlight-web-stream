import { DetailedHost, UndetailedHost } from "./api_bindings.js";
import { Api, ASSETS, getApi, getDetailedHost, getHosts } from "./common.js";
import { Component, ComponentHost, ListComponent } from "./gui/component.js";
import { setContextMenu } from "./gui/contextmenu.js";
import { showErrorPopup } from "./gui/error.js";
import { showMessage } from "./gui/modal.js";

// TODO: error handler with popup

async function startApp() {
    const api = await getApi()

    const rootElement = document.getElementById("root");
    if (rootElement == null) {
        showErrorPopup("couldn't find root element", true)
        return;
    }

    const rootComponent = new HostList()
    const root = new ComponentHost(rootElement, rootComponent)

    rootComponent.forceUpdate(api)
}

console.log("starting app")
startApp()

class HostList implements Component {
    private list: ListComponent<Host>

    constructor(hosts?: UndetailedHost) {
        this.list = new ListComponent([], {
            listElementClasses: ["host-list"]
        })
    }

    async forceUpdate(api: Api) {
        const hosts = await getHosts(api)

        this.updateDisplay(hosts)
    }

    private updateDisplay(hosts: UndetailedHost[]) {
        hosts.forEach(host => {
            const hostComponent = this.list.get().find(listHost => listHost.getHostId() == host.host_id)

            if (hostComponent) {
                hostComponent.updateDisplay(host)
            } else {
                const newHost = new Host(host.host_id, host)
                this.list.append(newHost)
            }
        })

        // remove old hosts
        for (let i = 0; i < this.list.get().length; i++) {
            const hostComponent = this.list.get()[i]

            const hostExists = hosts.findIndex(host => host.host_id == hostComponent.getHostId()) != -1
            if (!hostExists) {
                this.list.remove(i)
                // decrement i because we'll add one in the loop
                // however the removed element must be accounted
                i--
            }
        }
    }

    getHost(hostId: number) { }

    mount(parent: Element): void {
        this.list.mount(parent)
    }
    unmount(parent: Element): void {
        this.list.unmount(parent)
    }
}

class Host implements Component {
    private hostId: number
    private host: UndetailedHost | null = null
    private detailedHost: DetailedHost | null = null

    private divElement = document.createElement("div")

    private imageElement: HTMLImageElement = document.createElement("img")
    private imageOverlayElement: HTMLImageElement = document.createElement("img")
    private nameElement: HTMLElement = document.createElement("p")

    constructor(hostId: number, host?: UndetailedHost) {
        this.hostId = hostId
        this.host = host ?? null


        // Configure image
        this.imageElement.classList.add("host-image")
        this.imageElement.src = ASSETS.HOST_IMAGE

        // Configure image overlay
        this.imageOverlayElement.classList.add("host-image-overlay")
        this.imageOverlayElement.src = ASSETS.HOST_OVERLAY_LOCK

        // Configure name
        this.nameElement.classList.add("host-name")

        // Configure div
        this.divElement.classList.add("host-background")
        this.divElement.appendChild(this.imageElement)
        this.divElement.appendChild(this.imageOverlayElement)
        this.divElement.appendChild(this.nameElement)
        this.divElement.addEventListener("contextmenu", event => {
            setContextMenu(event, {
                elements: [{
                    name: "Show Details",
                    callback: async () => {
                        let host = this.detailedHost;
                        if (!host) {
                            const api = await getApi()
                            host = await getDetailedHost(api, this.hostId)
                        }
                        if (!host) {
                            showErrorPopup(`failed to get details for host ${this.hostId}`)
                            return;
                        }
                        this.detailedHost = host;

                        await showMessage(
                            `Web Id: ${host.host_id}\n` +
                            `Name: ${host.name}\n` +
                            `Pair Status: ${host.paired}\n` +
                            `State: ${host.server_state}\n` +
                            `Https Port: ${host.https_port}\n` +
                            `External Port: ${host.external_port}\n` +
                            `Version: ${host.version}\n` +
                            `Gfe Version: ${host.gfe_version}\n` +
                            `Unique ID: ${host.unique_id}\n` +
                            `MAC: ${host.mac}\n` +
                            `Local IP: ${host.local_ip}\n` +
                            `Current Game: ${host.current_game}\n` +
                            `Max Luma Pixels Hevc: ${host.max_luma_pixels_hevc}\n` +
                            `Server Codec Mode Support: ${host.server_codec_mode_support}`
                        )
                    }
                }, {
                    name: "Remove Host",
                    callback: async => {
                        // TODO
                    }
                }]
            })
        })

        this.updateDisplay()
    }

    getHostId(): number {
        return this.hostId
    }

    updateDisplay(host?: UndetailedHost) {
        this.nameElement.innerText = this.host?.name ?? "! Unknown !"

        // TODO: update image
    }

    mount(parent: Element): void {
        parent.appendChild(this.divElement)
    }
    unmount(parent: Element): void {
        parent.removeChild(this.divElement)
    }
}
