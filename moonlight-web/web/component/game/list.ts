import { Api, apiGetApps } from "../../api.js";
import { App } from "../../api_bindings.js";
import { Component } from "../index.js";
import { ListComponent } from "../list.js";
import { Game } from "./index.js";

export class GameList implements Component {
    private api: Api

    private hostId: number

    private list: ListComponent<Game>

    private cache: App | null = null

    constructor(api: Api, hostId: number, cache: App[] | null) {
        this.api = api

        this.hostId = hostId

        // List component
        this.list = new ListComponent([], {
            componentDivClasses: ["app-list"],
            listElementClasses: ["app-element"]
        })

        // Update cache
        if (cache != null) {
            this.updateCache(cache)
        } else {
            this.forceFetch()
        }
    }

    async forceFetch() {
        const apps = await apiGetApps(this.api, {
            host_id: this.hostId
        })

        this.updateCache(apps)
    }

    updateCache(cache: App[] | null) {
        // TODO: fix
    }

    getHostId(): number {
        return this.hostId
    }

    mount(parent: HTMLElement): void {
        this.list.mount(parent)
    }
    unmount(parent: HTMLElement): void {
        this.list.unmount(parent)
    }
}