declare namespace Panel {

    interface Info extends ItemInfo {

    }

    interface ItemInfo extends Common.InfoBase {
        icon: ItemIcon |null
        title: string
        url: string
        sort?: number
        lanUrl?: string
        mobileUrl?: string
        description?: string
        openMethod: number
        itemIconGroupId ?:number
    }

    interface ItemIconGroup extends Common.InfoBase {
        icon?: string
        title?: string
        sort?:number
        parentId?: number | null
        /** Server-reported item count; absent when the group is empty. */
        itemCount?: number
    }

    interface ItemIcon {
        itemType: number
        src ?: string
        fileName?: string
        text ?: string
        // bgColor ?: string
        backgroundColor ?: string
    }

    interface State {
        rightSiderCollapsed: boolean
        leftSiderCollapsed: boolean
        networkMode:PanelStateNetworkModeEnum | null
        panelConfig:panelConfig
        /** The config the server last confirmed. The appearance and layout pages
         *  compare against it to tell the user what is still unsaved; null means
         *  no server answer has arrived yet, so nothing is claimed to be dirty. */
        savedPanelConfig?:panelConfig | null
        /** The active Space's default wallpaper, resolved by the home whenever
         *  the Space changes. A runtime cache, not a user preference: the
         *  wallpaper layer only falls back to it while the visitor has not
         *  picked a wallpaper of their own. */
        spaceWallpaper?:SpaceWallpaperConfig | null
    }

    interface SpaceWallpaperConfig {
        mode:'theme' | 'library' | 'custom' | 'none'
        kind?:string
        source?:string
        poster?:string
        libraryEntry?:string
    }

    interface panelConfig{
        backgroundImageSrc?:string
        wallpaperMode?:'theme' | 'custom' | 'library' | 'none'
        wallpaperKind?:'image' | 'video' | 'webBundle' | 'externalUrl'
        wallpaperSource?:string
        wallpaperPoster?:string
        /** The Core wallpaper library entry the user picked (see
         *  `wallpaperMode: 'library'`). Only an id: the layer resolves it against
         *  the library, so a stored choice survives a library rebuild. */
        wallpaperLibraryEntry?:string
        backgroundBlur?:number
        /** 首页表面的统一透明度（0.4–1）。外观页滑杆写入，缺省 0.72。 */
        surfaceOpacity?:number
        backgroundMaskNumber?:number
        iconStyle?:PanelPanelConfigStyleEnum
        iconTextColor?:string
        useThemeDefaults?:boolean
        iconTextInfoHideDescription?:boolean
        iconTextIconHideTitle?:boolean
        logoText?:string
        logoImageSrc?:string
        clockShowSecond?:boolean
        clockColor?:string
        searchBoxShow?:boolean
        searchBoxSearchIcon?:boolean
        marginTop?:number
        marginBottom?:number
        maxWidth?:number
        maxWidthUnit:string
        marginX?:number
        footerHtml?:string
        systemMonitorShow?:boolean
        systemMonitorShowTitle?:boolean
        netModeChangeButtonShow?:boolean
    }

    interface userConfig{
        panel?:panelConfig
        language?:string
    }

    interface ItemIconSortRequest{
        sortItems:Common.SortItemRequest[]
        itemIconGroupId:number
    }
}
