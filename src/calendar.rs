use chrono::{Datelike, Duration, Local, NaiveDate, Weekday};

/// カレンダーの各マス（セル）を表す構造体
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalendarCell {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub is_current_month: bool,
    pub is_today: bool,
    pub weekday: Weekday,
}

/// カレンダーの表示状態およびロジックを管理するモデル
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarModel {
    pub view_year: i32,
    pub view_month: u32,
    pub today: NaiveDate,
}

impl CalendarModel {
    /// システムの現在日時を基準として初期化
    pub fn new() -> Self {
        let today = Local::now().date_naive();
        Self {
            view_year: today.year(),
            view_month: today.month(),
            today,
        }
    }

    /// テストや特定の日付を基準とする初期化用
    pub fn with_date(today: NaiveDate, view_year: i32, view_month: u32) -> Self {
        Self {
            view_year,
            view_month,
            today,
        }
    }

    /// 前月に移動
    pub fn prev_month(&mut self) {
        if self.view_month == 1 {
            self.view_year -= 1;
            self.view_month = 12;
        } else {
            self.view_month -= 1;
        }
    }

    /// 翌月に移動
    pub fn next_month(&mut self) {
        if self.view_month == 12 {
            self.view_year += 1;
            self.view_month = 1;
        } else {
            self.view_month += 1;
        }
    }

    /// 現在日時の月へ表示をリセット
    pub fn go_to_today(&mut self) {
        self.refresh_today();
        self.view_year = self.today.year();
        self.view_month = self.today.month();
    }

    /// システムの現在日付を再取得
    pub fn refresh_today(&mut self) {
        self.today = Local::now().date_naive();
    }

    /// 6行×7列（計42マス）の日曜日始まりのカレンダーグリッドを生成
    pub fn generate_grid(&self) -> [CalendarCell; 42] {
        let first_day_of_month = NaiveDate::from_ymd_opt(self.view_year, self.view_month, 1)
            .unwrap_or_else(|| NaiveDate::from_ymd_opt(self.view_year, 1, 1).unwrap());

        // 日曜日始まり: 日曜=0, 月曜=1, ..., 土曜=6
        let days_before = first_day_of_month.weekday().num_days_from_sunday() as i64;
        let start_date = first_day_of_month - Duration::days(days_before);

        let mut grid: [CalendarCell; 42] = [CalendarCell {
            year: self.view_year,
            month: self.view_month,
            day: 1,
            is_current_month: true,
            is_today: false,
            weekday: Weekday::Sun,
        }; 42];

        for i in 0..42 {
            let current_date = start_date + Duration::days(i as i64);
            let year = current_date.year();
            let month = current_date.month();
            let day = current_date.day();
            let is_current_month = year == self.view_year && month == self.view_month;
            let is_today = current_date == self.today;
            let weekday = current_date.weekday();

            grid[i] = CalendarCell {
                year,
                month,
                day,
                is_current_month,
                is_today,
                weekday,
            };
        }

        grid
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calendar_grid_counts_and_continuity() {
        let today = NaiveDate::from_ymd_opt(2026, 10, 12).unwrap();
        let cal = CalendarModel::with_date(today, 2026, 10);
        let grid = cal.generate_grid();

        assert_eq!(grid.len(), 42);

        // 2026年10月1日は木曜日
        // 日曜始まりなので、先頭セルは前月(9月)の27日(日)になるはず
        assert_eq!(grid[0].year, 2026);
        assert_eq!(grid[0].month, 9);
        assert_eq!(grid[0].day, 27);
        assert_eq!(grid[0].weekday, Weekday::Sun);
        assert!(!grid[0].is_current_month);

        // 10月1日は4番目（インデックス4）
        assert_eq!(grid[4].year, 2026);
        assert_eq!(grid[4].month, 10);
        assert_eq!(grid[4].day, 1);
        assert_eq!(grid[4].weekday, Weekday::Thu);
        assert!(grid[4].is_current_month);

        // 10月31日はインデックス34 (4 + 30)
        assert_eq!(grid[34].year, 2026);
        assert_eq!(grid[34].month, 10);
        assert_eq!(grid[34].day, 31);
        assert_eq!(grid[34].weekday, Weekday::Sat);
        assert!(grid[34].is_current_month);

        // 11月1日はインデックス35
        assert_eq!(grid[35].year, 2026);
        assert_eq!(grid[35].month, 11);
        assert_eq!(grid[35].day, 1);
        assert_eq!(grid[35].weekday, Weekday::Sun);
        assert!(!grid[35].is_current_month);

        // 最終セルは11月7日(土)
        assert_eq!(grid[41].year, 2026);
        assert_eq!(grid[41].month, 11);
        assert_eq!(grid[41].day, 7);
        assert_eq!(grid[41].weekday, Weekday::Sat);
        assert!(!grid[41].is_current_month);
    }

    #[test]
    fn test_prev_and_next_month() {
        let today = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
        let mut cal = CalendarModel::with_date(today, 2026, 1);

        // 1月の前月は前年12月
        cal.prev_month();
        assert_eq!(cal.view_year, 2025);
        assert_eq!(cal.view_month, 12);

        // 12月の翌月は翌年1月
        cal.next_month();
        assert_eq!(cal.view_year, 2026);
        assert_eq!(cal.view_month, 1);

        cal.next_month();
        assert_eq!(cal.view_year, 2026);
        assert_eq!(cal.view_month, 2);
    }

    #[test]
    fn test_today_highlight() {
        let today = NaiveDate::from_ymd_opt(2026, 10, 12).unwrap();
        let cal = CalendarModel::with_date(today, 2026, 10);
        let grid = cal.generate_grid();

        let today_cells: Vec<&CalendarCell> = grid.iter().filter(|c| c.is_today).collect();
        assert_eq!(today_cells.len(), 1);
        assert_eq!(today_cells[0].year, 2026);
        assert_eq!(today_cells[0].month, 10);
        assert_eq!(today_cells[0].day, 12);

        // 翌月を表示している場合、グリッド内に今日の日付は存在しない
        let cal_next = CalendarModel::with_date(today, 2026, 11);
        let grid_next = cal_next.generate_grid();
        let today_cells_next: Vec<&CalendarCell> = grid_next.iter().filter(|c| c.is_today).collect();
        assert_eq!(today_cells_next.len(), 0);
    }

    #[test]
    fn test_leap_year_february() {
        // 2024年はうるう年（2月29日がある）
        let today = NaiveDate::from_ymd_opt(2024, 2, 1).unwrap();
        let cal = CalendarModel::with_date(today, 2024, 2);
        let grid = cal.generate_grid();

        let feb29_cell = grid
            .iter()
            .find(|c| c.year == 2024 && c.month == 2 && c.day == 29);
        assert!(feb29_cell.is_some());
        assert!(feb29_cell.unwrap().is_current_month);
    }

    #[test]
    fn test_go_to_today() {
        let today = NaiveDate::from_ymd_opt(2026, 10, 12).unwrap();
        let mut cal = CalendarModel::with_date(today, 2025, 5);
        assert_eq!(cal.view_year, 2025);
        assert_eq!(cal.view_month, 5);

        // view_year / view_month が today にリセットされる
        cal.view_year = cal.today.year();
        cal.view_month = cal.today.month();
        assert_eq!(cal.view_year, 2026);
        assert_eq!(cal.view_month, 10);
    }
}
