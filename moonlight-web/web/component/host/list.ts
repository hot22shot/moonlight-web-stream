import { DetailedHost, UndetailedHost } from "../../api_bindings.js"
import { Api, apiGetHosts } from "../../api.js"
import { Component, ComponentEvent } from "../index.js"
import { Host, HostEventListener } from "./index.js"
import { ListComponent } from "../list.js"

export class HostList implements Component {
    private api: Api

    private eventTarget = new EventTarget()
    private list: ListComponent<Host>

    constructor(api: Api) {
        this.api = api

        this.list = new ListComponent([], {
            listElementClasses: ["host-list"],
            componentDivClasses: ["host-element"]
        })
    }

    async forceFetch() {
        const hosts = await apiGetHosts(this.api)

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
            newHost.addHostOpenListener(this.onHostOpenEvent.bind(this))
        }
    }
    removeHost(hostId: number) {
        const index = this.list.get().findIndex(listHost => listHost.getHostId() == hostId)

        if (index != -1) {
            const hostComponent = this.list.remove(index)

            hostComponent?.addHostOpenListener(this.onHostOpenEvent.bind(this))
            hostComponent?.removeHostRemoveListener(this.removeHostListener.bind(this))
        }
    }
    getHost(hostId: number): Host | undefined {
        return this.list.get().find(host => host.getHostId() == hostId)
    }

    private onHostOpenEvent(event: ComponentEvent<Host>) {
        this.eventTarget.dispatchEvent(new ComponentEvent("ml-hostopen", event.component))
    }

    addHostOpenListener(listener: HostEventListener, options?: EventListenerOptions) {
        this.eventTarget.addEventListener("ml-hostopen", listener as EventListenerOrEventListenerObject, options)
    }
    removeHostOpenListener(listener: HostEventListener, options?: EventListenerOptions) {
        this.eventTarget.removeEventListener("ml-hostopen", listener as EventListenerOrEventListenerObject, options)
    }

    mount(parent: Element): void {
        this.list.mount(parent)
    }
    unmount(parent: Element): void {
        this.list.unmount(parent)
    }
}