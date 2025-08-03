import { Api, apiGetApps } from "../../api.js";
import { App } from "../../api_bindings.js";
import { showErrorPopup } from "../error.js";
import { FetchListComponent } from "../fetch_list.js";
import { Game } from "./index.js";

// TODO: move to fetch list
export class GameList extends FetchListComponent<App, Game> {
    private api: Api

    private hostId: number

    constructor(api: Api, hostId: number, cache: App[] | null) {
        super({
            listClasses: ["app-list"],
            elementDivClasses: ["animated-list-element", "app-element"]
        })

        this.api = api

        this.hostId = hostId

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

        if (apps) {
            this.updateCache(apps)
        } else {
            showErrorPopup(`failed to fetch apps for host ${this.getHostId()}`)
        }
    }

    protected updateComponentData(component: Game, data: App): void {
        component.updateCache(data)
    }
    protected getComponentDataId(component: Game): number {
        return component.getAppId()
    }
    protected getDataId(data: App): number {
        return data.app_id
    }
    protected insertList(dataId: number, data: App): void {
        this.list.append(new Game(this.api, this.hostId, dataId, data))
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