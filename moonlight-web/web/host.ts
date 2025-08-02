import { DetailedHost, PutHostRequest, UndetailedHost } from "./api_bindings.js"
import { Api, ASSETS, deleteHost, getApi, getHost, getHosts, postPair } from "./common.js"
import { Component, ComponentEvent, ListComponent } from "./gui/component.js"
import { setContextMenu } from "./gui/context_menu.js"
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

    constructor(api: Api) {
        this.api = api

        this.list = new ListComponent([], {
            listElementClasses: ["host-list"],
            componentDivClasses: ["host-element"]
        })
    }

    async forceFetch() {
        const hosts = await getHosts(this.api)

        this.updateCache(hosts)
    }

    private updateCache(hosts: UndetailedHost[]) {
        // add new hosts and update old ones
        hosts.forEach(host => {
            this.insertUpdateHost(host)
        })

        // remove old hosts
        for (let i = 0; i < this.list.get().length; i++) {
            const hostComponent = this.list.get()[i]

            const hostExists = hosts.findIndex(host => host.host_id == hostComponent.getHostId()) != -1
            if (!hostExists) {
                this.removeHost(hostComponent.getHostId())
                // decrement i because we'll add one in the loop
                // however the removed element must be accounted
                i--
            }
        }
    }

    private removeHostListener(event: ComponentEvent<Host>) {
        this.removeHost(event.component.getHostId())
    }

    insertUpdateHost(host: UndetailedHost | DetailedHost) {
        const hostComponent = this.list.get().find(listHost => listHost.getHostId() == host.host_id)

        if (hostComponent) {
            hostComponent.updateCache(host)
        } else {
            const newHost = new Host(this.api, host.host_id, host)
            this.list.append(newHost)

            newHost.addHostRemoveListener(this.removeHostListener.bind(this))
        }
    }
    removeHost(hostId: number) {
        const index = this.list.get().findIndex(listHost => listHost.getHostId() == hostId)

        if (index != -1) {
            const hostComponent = this.list.remove(index)

            hostComponent?.removeHostRemoveListener(this.removeHostListener.bind(this))
        }
    }
    getHost(hostId: number): Host | undefined {
        return this.list.get().find(host => host.getHostId() == hostId)
    }

    mount(parent: Element): void {
        this.list.mount(parent)
    }
    unmount(parent: Element): void {
        this.list.unmount(parent)
    }
}

export type HostRemoveEventListener = (event: ComponentEvent<Host>) => void

export class Host implements Component {
    private api: Api

    private hostId: number
    private cache: UndetailedHost | DetailedHost | null = null

    private divElement: HTMLDivElement = document.createElement("div")

    private imageElement: HTMLImageElement = document.createElement("img")
    private imageOverlayElement: HTMLImageElement = document.createElement("img")
    private nameElement: HTMLElement = document.createElement("p")

    constructor(api: Api, hostId: number, host: UndetailedHost | DetailedHost | null) {
        this.api = api

        this.hostId = hostId
        this.cache = host

        // Configure image
        this.imageElement.classList.add("host-image")
        this.imageElement.src = ASSETS.HOST_IMAGE

        // Configure image overlay
        this.imageOverlayElement.classList.add("host-image-overlay")

        // Configure name
        this.nameElement.classList.add("host-name")

        // Append elements
        this.divElement.appendChild(this.imageElement)
        this.divElement.appendChild(this.imageOverlayElement)
        this.divElement.appendChild(this.nameElement)

        this.divElement.addEventListener("click", this.onClick.bind(this))
        this.divElement.addEventListener("contextmenu", this.onContextMenu.bind(this))

        // Update elements
        if (host != null) {
            this.updateCache(host)
        } else {
            this.forceFetch()
        }
    }

    async forceFetch() {
        const newCache = await getHost(this.api, this.hostId)
        if (newCache == null) {
            showErrorPopup(`failed to fetch host ${this.getHostId()}`)
            return;
        }

        this.updateCache(newCache)
    }

    private async onClick() {
        if (this.cache?.paired == "Paired") {
            // TODO: go into games view
        } else {
            await this.pair()
        }
    }

    private onContextMenu(event: MouseEvent) {
        const elements = []

        elements.push({
            name: "Show Details",
            callback: this.showDetails.bind(this),
        })

        if (this.cache?.paired == "NotPaired") {
            elements.push({
                name: "Pair",
                callback: this.pair.bind(this)
            })
        }

        elements.push({
            name: "Remove Host",
            callback: this.remove.bind(this)
        })

        setContextMenu(event, {
            elements
        })
    }

    private async showDetails() {
        let host = this.cache;
        if (!host || !isDetailedHost(host)) {
            const api = await getApi()
            host = await getHost(api, this.hostId)
        }
        if (!host || !isDetailedHost(host)) {
            showErrorPopup(`failed to get details for host ${this.hostId}`)
            return;
        }
        this.updateCache(host)

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

    addHostRemoveListener(listener: HostRemoveEventListener, options?: EventListenerOptions) {
        this.divElement.addEventListener("ml-hostremove", listener as EventListenerOrEventListenerObject, options)
    }
    removeHostRemoveListener(listener: HostRemoveEventListener, options?: EventListenerOptions) {
        this.divElement.removeEventListener("ml-hostremove", listener as EventListenerOrEventListenerObject, options)
    }

    private async remove() {
        const success = await deleteHost(this.api, {
            host_id: this.getHostId()
        })

        if (!success) {
            showErrorPopup(`something went wrong whilst removing the host ${this.getHostId()}`)
        }
        this.divElement.dispatchEvent(new ComponentEvent("ml-hostremove", this))
    }
    private async pair() {
        if (this.cache?.paired == "Paired") {
            await this.forceFetch()

            if (this.cache?.paired == "Paired") {
                showMessage("This host is already paired!")
                return;
            }
        }

        const pinResponse = await postPair(this.api, {
            host_id: this.getHostId()
        })
        if (pinResponse == null) {
            showErrorPopup("failed to pair host")
            return
        }
        if ("error" in pinResponse) {
            showErrorPopup(`failed to pair host: ${pinResponse.error}`)
            return
        }

        const messageAbort = new AbortController()
        showMessage(`Please pair your host ${this.getCache()?.name} with this pin:\nPin: ${pinResponse.pin}`, { signal: messageAbort.signal })

        const resultResponse = await pinResponse.result
        messageAbort.abort()

        if (resultResponse == null) {
            showErrorPopup("failed to pair to host: Make sure the Pin is correct")
            return;
        }

        this.updateCache(resultResponse)
    }

    getHostId(): number {
        return this.hostId
    }

    getCache(): DetailedHost | UndetailedHost | null {
        return this.cache
    }

    updateCache(host: UndetailedHost | DetailedHost) {
        if (this.getHostId() != host.host_id) {
            showErrorPopup(`tried to overwrite host ${this.getHostId()} with data from ${host.host_id}`)
            return
        }

        if (this.cache == null) {
            this.cache = host
        } else {
            Object.assign(this.cache, host)
        }

        // Update Elements
        this.nameElement.innerText = this.cache.name

        if (this.cache.paired != "Paired") {
            this.imageOverlayElement.src = ASSETS.HOST_OVERLAY_LOCK
        } else {
            this.imageOverlayElement.src = ASSETS.HOST_OVERLAY_NONE
        }
    }

    mount(parent: HTMLElement): void {
        parent.appendChild(this.divElement)
    }
    unmount(parent: HTMLElement): void {
        parent.removeChild(this.divElement)
    }
}