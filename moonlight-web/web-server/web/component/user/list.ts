import { User } from "./index.js";
import { DetailedUser } from "../../api_bindings.js";
import { FetchListComponent } from "../fetch_list.js";

export class UserList extends FetchListComponent<DetailedUser, User> {

    async forceFetch(forceServerRefresh?: boolean): Promise<void> {

    }

    protected insertList(dataId: number, data: DetailedUser): void {
        // TODO
    }

    protected updateComponentData(component: User, data: DetailedUser): void {

    }
    protected getDataId(data: DetailedUser): number {
        // TODO
        return 0
    }
    protected getComponentDataId(component: User): number {
        // TODO
        return 0
    }
}