use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn render(f: &mut Frame, area: Rect) {
    // Centro el modal: 60% ancho, ~80% alto
    let popup = centered_rect(62, 82, area);

    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(Span::styled(
            " Ayuda — Atajos de teclado  [F1 / Esc para cerrar] ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let sections: Vec<(&str, Vec<(&str, &str)>)> = vec![
        (
            "Navegación general",
            vec![
                ("[q]",   "Salir de rtop"),
                ("[Ctrl+C]", "Salir (siempre)"),
                ("[Tab]", "Cambiar pestaña (Procesos ↔ Contenedores)"),
                ("[F1]",  "Mostrar / cerrar esta ayuda"),
                ("[Esc]", "Cerrar modal / salir de vista de detalle"),
            ],
        ),
        (
            "Sistema",
            vec![
                ("[[]]",   "Disminuir intervalo de refresco"),
                ("[]]]",   "Aumentar intervalo de refresco"),
                ("[F2]",   "Selector de disco"),
                ("[F3]",   "Selector de interfaz de red"),
            ],
        ),
        (
            "Procesos",
            vec![
                ("[↑ / ↓]", "Navegar lista"),
                ("[Enter]", "Ver detalle del proceso"),
                ("[/]",     "Activar filtro por nombre"),
                ("[Esc]",   "Limpiar filtro"),
                ("[c]",     "Ordenar por CPU"),
                ("[m]",     "Ordenar por Memoria"),
                ("[r]",     "Ordenar por Lectura de disco"),
                ("[w]",     "Ordenar por Escritura de disco"),
            ],
        ),
        (
            "Contenedores",
            vec![
                ("[↑ / ↓]", "Navegar lista"),
                ("[Enter]", "Ver detalle del contenedor"),
                ("[l]",     "Ver logs del contenedor"),
                ("[r]",     "Reiniciar contenedor"),
                ("[s]",     "Parar contenedor"),
            ],
        ),
        (
            "Logs de contenedor",
            vec![
                ("[↑ / ↓]", "Desplazar logs"),
                ("[f]",     "Activar / desactivar seguimiento automático"),
                ("[Esc]",   "Volver al detalle del contenedor"),
            ],
        ),
    ];

    // Divide el inner en columnas para las secciones
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Length(1); inner.height as usize])
        .split(inner);

    let mut line_idx: usize = 0;
    let render_line = |f: &mut Frame, rect: Rect, line: Line| {
        f.render_widget(Paragraph::new(line), rect);
    };

    for (section_title, bindings) in &sections {
        if line_idx >= rows.len() {
            break;
        }
        render_line(
            f,
            rows[line_idx],
            Line::from(vec![Span::styled(
                format!(" {} ", section_title),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            )]),
        );
        line_idx += 1;

        for (key, desc) in bindings {
            if line_idx >= rows.len() {
                break;
            }
            render_line(
                f,
                rows[line_idx],
                Line::from(vec![
                    Span::styled(
                        format!("  {:12}", key),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(*desc, Style::default().fg(Color::White)),
                ]),
            );
            line_idx += 1;
        }

        // Separador vacío entre secciones
        if line_idx < rows.len() {
            render_line(f, rows[line_idx], Line::from(""));
            line_idx += 1;
        }
    }

    // Pie
    let footer_line = Line::from(vec![Span::styled(
        "  Versión rtop 0.1",
        Style::default().fg(Color::DarkGray),
    )])
    .alignment(Alignment::Center);
    if line_idx < rows.len() {
        f.render_widget(Paragraph::new(footer_line), rows[line_idx]);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
