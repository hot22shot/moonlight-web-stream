import { DetailedHost, PutHostRequest, UndetailedHost } from "./api_bindings.js"
import { Api, ASSETS, deleteHost, getApi, getHost, getHosts } from "./common.js"
import { Component, ListComponent } from "./gui/component.js"
import { setContextMenu } from "./gui/contextmenu.js"
import { showErrorPopup } from "./gui/error.js"
import { FormModal, showMessage } from "./gui/modal.js"

export function isDetailedHost(host: UndetailedHost | DetailedHost): host is DetailedHost {
    return (host as DetailedHost).https_port !== undefined
}

export class AddHostModal extends FormModal<PutHostRequest> {

    private addressElement: HTMLInputElement = document.createElement("input")
    private httpPortElement: HTMLInputElement = document.createElement("input")

    constructor() {
        super()

        this.addressElement.type = "text"

        this.httpPortElement.type = "text"
        this.httpPortElement.inputMode = "numeric"
    }

    reset(): void {
        this.addressElement.value = ""
        this.httpPortElement.value = ""
    }
    submit(): PutHostRequest | null {
        const address = this.addressElement.value
        const httpPort = this.httpPortElement.valueAsNumber

        return {
            address,
            http_port: httpPort
        }
    }

    mountForm(form: HTMLFormElement): void {
        form.appendChild(this.addressElement)
        form.appendChild(this.httpPortElement)
    }
}

export class HostList implements Component {
    private api: Api

    private list: ListComponent<Host>

    constructor(api: Api, hosts?: UndetailedHost) {
        this.api = api

        this.list = new ListComponent([], {
            listElementClasses: ["host-list"],
            componentDivClasses: ["host-element"]
        })
    }

    async forceFetch() {
        const hosts = await getHosts(this.api)

        this.updateDisplay(hosts)
    }

    private updateDisplay(hosts: UndetailedHost[]) {
        // add new hosts and update old ones
        hosts.forEach(host => {
            this.insertHost(host)
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

    insertHost(host: UndetailedHost) {
        const hostComponent = this.list.get().find(listHost => listHost.getHostId() == host.host_id)

        if (hostComponent) {
            hostComponent.updateDisplay(host)
        } else {
            const newHost = new Host(this.api, host.host_id, host)
            this.list.append(newHost)
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

export class Host implements Component {
    private api: Api

    private hostId: number
    private host: UndetailedHost | DetailedHost | null = null

    private imageElement: HTMLImageElement = document.createElement("img")
    private imageOverlayElement: HTMLImageElement = document.createElement("img")
    private nameElement: HTMLElement = document.createElement("p")

    constructor(api: Api, hostId: number, host?: UndetailedHost) {
        this.api = api

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

        this.updateDisplay()
    }

    private onContextMenu(event: MouseEvent) {
        setContextMenu(event, {
            elements: [{
                name: "Show Details",
                callback: this.showDetails.bind(this),
            }, {
                name: "Remove Host",
                callback: this.removeHost.bind(this)
            }]
        })
    }

    private async showDetails() {
        let host = this.host;
        if (!host || !isDetailedHost(host)) {
            const api = await getApi()
            host = await getHost(api, this.hostId)
        }
        if (!host || !isDetailedHost(host)) {
            showErrorPopup(`failed to get details for host ${this.hostId}`)
            return;
        }
        this.host = host;

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

    private async removeHost() {
        const success = await deleteHost(this.api, {
            host_id: this.getHostId()
        })

        if (!success) {
            showErrorPopup(`something went wrong whilst removing the host ${this.getHostId()}`)
        }
    }

    getHostId(): number {
        return this.hostId
    }

    updateDisplay(host?: UndetailedHost) {
        this.nameElement.innerText = this.host?.name ?? "! Unknown !"

        // TODO: update image
    }

    mount(parent: HTMLElement): void {
        parent.appendChild(this.imageElement)
        parent.appendChild(this.imageOverlayElement)
        parent.appendChild(this.nameElement)

        parent.addEventListener("contextmenu", this.onContextMenu.bind(this))
    }
    unmount(parent: HTMLElement): void {
        parent.removeChild(this.imageElement)
        parent.removeChild(this.imageOverlayElement)
        parent.removeChild(this.nameElement)

        parent.removeEventListener("contextmenu", this.onContextMenu.bind(this))
    }
}