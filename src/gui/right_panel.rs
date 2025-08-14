use crate::gui::view::{ItemListStyle, MenuStyle};
use crate::gui::{FileTreeApp, Message, RightPanelFile, SortColumn, SortOrder};
use crate::utils::format_duration;
use iced::{
    Element, Length,
    widget::{Scrollable, Space},
};

#[derive(Default)]
struct AudioColumnToggles {
    show_creator: bool,
    show_album: bool,
    show_title: bool,
    show_genre: bool,
    show_duration: bool,
}

/// Creates a widget displaying the total number of items and the sum of
/// durations for all files shown in the right panel.
fn create_totals_display(
    displayed_files: &[RightPanelFile],
    menu_style: MenuStyle,
) -> Element<'static, Message> {
    let total_duration_ms: u64 =
        displayed_files.iter().filter_map(|f| f.duration_ms).sum();
    let row_count = displayed_files.len();
    let total_duration_str = format!(
        " {} Item{}, Time: {}",
        row_count,
        if row_count == 1 { "" } else { "s" },
        format_duration(Some(total_duration_ms)),
    );
    iced::widget::text(total_duration_str)
        .size(menu_style.text_size)
        .style(move |_theme| iced::widget::text::Style {
            color: Some([1.0, 1.0, 1.0, 1.0].into()),
        })
        .into()
}

/// Creates the right panel's menu row with "Shuffle", "Export to XSPF", and
/// "Play in VLC" buttons, applying the specified text size, spacing, and color
/// styling to each button.
fn create_right_panel_menu_row(
    menu_style: MenuStyle,
    extra_widget: Option<Element<'static, Message>>,
) -> Element<'static, Message> {
    let shuffle_button = iced::widget::button(
        iced::widget::text("Shuffle")
            .width(Length::Shrink)
            .size(menu_style.text_size)
            .style(move |_theme| iced::widget::text::Style {
                color: Some(menu_style.text_color.into()),
            }),
    )
    .on_press(Message::ShuffleRightPanel)
    .width(Length::Shrink);

    let export_button = iced::widget::button(
        iced::widget::text("Export to XSPF")
            .width(Length::Shrink)
            .size(menu_style.text_size)
            .style(move |_theme| iced::widget::text::Style {
                color: Some(menu_style.text_color.into()),
            }),
    )
    .on_press(Message::ExportRightPanelAsXspf)
    .width(Length::Shrink);

    let play_button = iced::widget::button(
        iced::widget::text("Play")
            .width(Length::Shrink)
            .size(menu_style.text_size)
            .style(move |_theme| iced::widget::text::Style {
                color: Some(menu_style.text_color.into()),
            }),
    )
    .on_press(Message::ExportAndPlayRightPanelAsXspf)
    .width(Length::Shrink);

    let mut row = iced::widget::Row::new()
        .push(shuffle_button)
        .push(export_button)
        .push(play_button)
        .spacing(menu_style.spacing);

    if let Some(widget) = extra_widget {
        row = row.push(Space::with_width(Length::Fill)).push(widget);
    }

    row.into()
}

/// Builds the header row for the right panel table, including sortable column
/// buttons for directory, file, and optionally creator, album, title, and
/// genre. Column spacing and text size are configurable via parameters.
fn create_right_panel_header_row(
    app: &FileTreeApp,
    audio_column_toggles: AudioColumnToggles,
    column_spacing: u16,
    header_text_size: u16,
    header_text_color: [f32; 4],
) -> iced::widget::Row<'static, Message> {
    // Sorting arrows
    let dir_arrow = if app.right_panel_sort_column == SortColumn::Directory {
        match app.right_panel_sort_order {
            SortOrder::Desc => " ↑",
            SortOrder::Asc => " ↓",
        }
    } else {
        ""
    };
    let file_arrow = if app.right_panel_sort_column == SortColumn::File {
        match app.right_panel_sort_order {
            SortOrder::Desc => " ↑",
            SortOrder::Asc => " ↓",
        }
    } else {
        ""
    };
    let creator_arrow = if audio_column_toggles.show_creator
        && app.right_panel_sort_column == SortColumn::Creator
    {
        match app.right_panel_sort_order {
            SortOrder::Desc => " ↑",
            SortOrder::Asc => " ↓",
        }
    } else {
        ""
    };
    let album_arrow = if audio_column_toggles.show_album
        && app.right_panel_sort_column == SortColumn::Album
    {
        match app.right_panel_sort_order {
            SortOrder::Desc => " ↑",
            SortOrder::Asc => " ↓",
        }
    } else {
        ""
    };
    let title_arrow = if audio_column_toggles.show_title
        && app.right_panel_sort_column == SortColumn::Title
    {
        match app.right_panel_sort_order {
            SortOrder::Desc => " ↑",
            SortOrder::Asc => " ↓",
        }
    } else {
        ""
    };
    let genre_arrow = if audio_column_toggles.show_genre
        && app.right_panel_sort_column == SortColumn::Genre
    {
        match app.right_panel_sort_order {
            SortOrder::Desc => " ↑",
            SortOrder::Asc => " ↓",
        }
    } else {
        ""
    };
    let duration_arrow = if app.right_panel_sort_column == SortColumn::Duration
    {
        match app.right_panel_sort_order {
            SortOrder::Desc => " ↑",
            SortOrder::Asc => " ↓",
        }
    } else {
        ""
    };

    let mut header_row = iced::widget::Row::new()
        .push(
            iced::widget::button(
                iced::widget::text(format!("Directory{dir_arrow}"))
                    .width(Length::FillPortion(1))
                    .size(header_text_size)
                    .style(move |_theme| iced::widget::text::Style {
                        color: Some(header_text_color.into()),
                    }),
            )
            .on_press(Message::SortRightPanelByDirectory)
            .width(Length::FillPortion(1)),
        )
        .push(
            iced::widget::button(
                iced::widget::text(format!("File{file_arrow}"))
                    .width(Length::FillPortion(1))
                    .size(header_text_size)
                    .style(move |_theme| iced::widget::text::Style {
                        color: Some(header_text_color.into()),
                    }),
            )
            .on_press(Message::SortRightPanelByFile)
            .width(Length::FillPortion(1)),
        );

    if audio_column_toggles.show_creator {
        header_row = header_row.push(
            iced::widget::button(
                iced::widget::text(format!("Musician{creator_arrow}"))
                    .width(Length::FillPortion(1))
                    .size(header_text_size)
                    .style(move |_theme| iced::widget::text::Style {
                        color: Some(header_text_color.into()),
                    }),
            )
            .on_press(Message::SortRightPanelByCreator)
            .width(Length::FillPortion(1)),
        );
    }
    if audio_column_toggles.show_album {
        header_row = header_row.push(
            iced::widget::button(
                iced::widget::text(format!("Album{album_arrow}"))
                    .width(Length::FillPortion(1))
                    .size(header_text_size)
                    .style(move |_theme| iced::widget::text::Style {
                        color: Some(header_text_color.into()),
                    }),
            )
            .on_press(Message::SortRightPanelByAlbum)
            .width(Length::FillPortion(1)),
        );
    }
    if audio_column_toggles.show_title {
        header_row = header_row.push(
            iced::widget::button(
                iced::widget::text(format!("Title{title_arrow}"))
                    .width(Length::FillPortion(1))
                    .size(header_text_size)
                    .style(move |_theme| iced::widget::text::Style {
                        color: Some(header_text_color.into()),
                    }),
            )
            .on_press(Message::SortRightPanelByTitle)
            .width(Length::FillPortion(1)),
        );
    }
    if audio_column_toggles.show_genre {
        header_row = header_row.push(
            iced::widget::button(
                iced::widget::text(format!("Genre{genre_arrow}"))
                    .width(Length::FillPortion(1))
                    .size(header_text_size)
                    .style(move |_theme| iced::widget::text::Style {
                        color: Some(header_text_color.into()),
                    }),
            )
            .on_press(Message::SortRightPanelByGenre)
            .width(Length::FillPortion(1)),
        );
    }
    if audio_column_toggles.show_duration {
        header_row = header_row.push(
            iced::widget::button(
                iced::widget::text(format!("Duration{duration_arrow}"))
                    .width(Length::FillPortion(1))
                    .size(header_text_size)
                    .style(move |_theme| iced::widget::text::Style {
                        color: Some(header_text_color.into()),
                    }),
            )
            .on_press(Message::SortRightPanelByDuration)
            .width(Length::FillPortion(1)),
        );
    }

    header_row = header_row.spacing(column_spacing);
    header_row
}

/// Creates the directory cell widget for a right panel row, displaying the
/// parent directory name with the specified text size and providing a context
/// menu for directory actions.
fn create_right_panel_directory_widget(
    file: &RightPanelFile,
    row_text_size: u16,
) -> Element<'static, Message> {
    let directory_name = file
        .path
        .parent()
        .and_then(|p| p.file_name())
        .map(|d| d.to_string_lossy().to_string())
        .unwrap_or_default();
    let directory_path = file.path.parent().map(|p| p.to_path_buf());
    let directory_widget = if let Some(path) = directory_path {
        let directory_path = path.clone();
        iced_aw::widgets::ContextMenu::new(
            iced::widget::text(directory_name.clone())
                .width(Length::FillPortion(1))
                .size(row_text_size),
            Box::new(move || {
                iced::widget::column![
                    iced::widget::button("Delete All in Directory").on_press(
                        Message::RemoveDirectoryFromRightPanel(
                            directory_path.clone()
                        )
                    )
                ]
                .into()
            }) as Box<dyn Fn() -> iced::Element<'static, Message>>,
        )
    } else {
        iced_aw::widgets::ContextMenu::new(
            iced::widget::text(directory_name.clone())
                .width(Length::FillPortion(1))
                .size(row_text_size),
            Box::new(|| iced::widget::column![].into())
                as Box<dyn Fn() -> iced::Element<'static, Message>>,
        )
    };
    directory_widget.into()
}

/// Creates the file cell widget for a right panel row, displaying the file name
/// with the  specified text size and providing a context menu for file-specific
/// actions.
fn create_right_panel_file_context_menu(
    file: &RightPanelFile,
    row_text_size: u16,
) -> Element<'static, Message> {
    let filename = file
        .path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();
    let file_context_menu = iced_aw::widgets::ContextMenu::new(
        iced::widget::text(filename.clone())
            .width(Length::FillPortion(1))
            .size(row_text_size),
        {
            let file_path = file.path.clone();
            Box::new(move || {
                iced::widget::column![
                    iced::widget::button("Delete").on_press(
                        Message::RemoveFromRightPanel(file_path.clone())
                    )
                ]
                .into()
            }) as Box<dyn Fn() -> iced::Element<'static, Message>>
        },
    );
    file_context_menu.into()
}

/// Assembles the entire right panel, including the menu row, header row, and
/// all file rows,  applying the specified menu size, spacing, and text color to
/// controls and table content.
pub(crate) fn create_right_panel(
    app: &FileTreeApp,
    menu_style: MenuStyle,
    item_list_style: ItemListStyle,
) -> Element<Message> {
    let displayed_files = app.sorted_right_panel_files();

    // Determine which columns to show
    let show_creator = displayed_files
        .iter()
        .any(|f| f.creator.as_ref().map(|s| !s.is_empty()).unwrap_or(false));
    let show_album = displayed_files
        .iter()
        .any(|f| f.album.as_ref().map(|s| !s.is_empty()).unwrap_or(false));
    let show_title = displayed_files
        .iter()
        .any(|f| f.title.as_ref().map(|s| !s.is_empty()).unwrap_or(false));
    let show_genre = displayed_files
        .iter()
        .any(|f| f.genre.as_ref().map(|s| !s.is_empty()).unwrap_or(false));
    let show_duration = displayed_files.iter().any(|f| f.duration_ms.is_some());

    let audio_column_toggles = AudioColumnToggles {
        show_creator,
        show_album,
        show_title,
        show_genre,
        show_duration,
    };

    let totals_display = create_totals_display(&displayed_files, menu_style);
    let header_text_size = item_list_style.row_text_size + 4;
    let menu_row =
        create_right_panel_menu_row(menu_style, Some(totals_display));

    let header_row = create_right_panel_header_row(
        app,
        audio_column_toggles,
        item_list_style.column_row_spacing,
        header_text_size,
        item_list_style.header_text_color,
    );

    let mut rows = Vec::new();
    for (i, file_ref) in displayed_files.iter().enumerate() {
        let file = file_ref.clone();

        let dir_widget = create_right_panel_directory_widget(
            &file,
            item_list_style.row_text_size,
        );
        let file_context_menu = create_right_panel_file_context_menu(
            &file,
            item_list_style.row_text_size,
        );

        let mut row =
            iced::widget::Row::new().push(dir_widget).push(file_context_menu);

        if show_creator {
            row = row.push(
                iced::widget::text(file.creator.clone().unwrap_or_default())
                    .width(Length::FillPortion(1))
                    .size(item_list_style.row_text_size),
            );
        }
        if show_album {
            row = row.push(
                iced::widget::text(file.album.clone().unwrap_or_default())
                    .width(Length::FillPortion(1))
                    .size(item_list_style.row_text_size),
            );
        }
        if show_title {
            row = row.push(
                iced::widget::text(file.title.clone().unwrap_or_default())
                    .width(Length::FillPortion(1))
                    .size(item_list_style.row_text_size),
            );
        }
        if show_genre {
            row = row.push(
                iced::widget::text(file.genre.clone().unwrap_or_default())
                    .width(Length::FillPortion(1))
                    .size(item_list_style.row_text_size),
            );
        }
        if show_duration {
            row = row.push(
                iced::widget::text(format_duration(file.duration_ms))
                    .width(Length::FillPortion(1))
                    .size(item_list_style.row_text_size),
            );
        }
        row = row.spacing(item_list_style.column_row_spacing);

        // Shade alternating pairs of rows
        let pair = (i / 2) % 2;
        let bg_color = if pair == 0 {
            // iced::Color::from_rgb(0.13, 0.13, 0.13) // darker
            iced::Color::from_rgb(
                item_list_style.dark_row_shade[0],
                item_list_style.dark_row_shade[1],
                item_list_style.dark_row_shade[2],
            )
        } else {
            // iced::Color::from_rgb(0.18, 0.18, 0.18) // lighter
            iced::Color::from_rgb(
                item_list_style.light_row_shade[0],
                item_list_style.light_row_shade[1],
                item_list_style.light_row_shade[2],
            )
        };

        let clickable_row = iced::widget::button(row)
            .on_press(Message::OpenRightPanelFile(file.path.clone()))
            .style(move |_theme, _style| iced::widget::button::Style {
                background: Some(iced::Background::Color(bg_color)),
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
                text_color: iced::Color::WHITE,
            });

        rows.push(clickable_row.into());
    }

    let col = iced::widget::Column::new()
        .push(Space::with_height(item_list_style.column_height_spacing))
        .push(menu_row)
        .push(Space::with_height(item_list_style.column_height_spacing))
        .push(header_row)
        .push(Scrollable::new(iced::widget::column(rows)));

    col.into()
}
