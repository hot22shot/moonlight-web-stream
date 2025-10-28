import { User } from "./index.js";
import { DetailedUser } from "../../api_bindings.js";
import { FetchListComponent } from "../fetch_list.js";
import { Api, apiGetUsers } from "../../api.js";

export class UserList extends FetchListComponent<DetailedUser, User> {
    private api: Api

    constructor(api: Api) {
        super({
            listClasses: ["user-list"],
            elementLiClasses: ["user-element"]
        })

        this.api = api
    }

    async forceFetch(forceServerRefresh?: boolean): Promise<void> {
        const response = await apiGetUsers(this.api)

        this.updateCache(response.users)
    }

    public insertList(dataId: number, data: DetailedUser): void {
        const newUser = new User(this.api, data)

        this.list.append(newUser)

        // TODO: add other listeners
    }

    setFilter(filter: string) {
        this.list.setFilter((user) =>
            user.getCache()?.name.includes(filter) ?? false
        )
    }

    protected updateComponentData(component: User, data: DetailedUser): void {
        component.updateCache(data)
    }

    protected getDataId(data: DetailedUser): number {
        return data.id
    }
    protected getComponentDataId(component: User): number {
        return component.getUserId()
    }
}