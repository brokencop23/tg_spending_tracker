use chrono::{DateTime, Datelike, Utc};


#[derive(Clone)]
pub struct Category {
    pub alias: String,
    pub name: String
}

impl Category {
    pub fn new(alias: String, name: String) -> Self {
        Self { alias, name }
    }
}

pub struct Item {
    date: DateTime<Utc>,
    category: Category,
    amount: f64,
}

impl Item {
    pub fn new(date: DateTime<Utc>, category: Category, amount: f64) -> Self {
        Self { date, category, amount }
    }
}

pub struct ItemCollection {
    items: Vec<Item>
}

pub struct ItemCollectionFilter<'a> {
    items: Vec<&'a Item>
}

pub struct ItemCollectionStat {
    n_items: usize
}

impl ItemCollection {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn from(items: Vec<Item>) -> Self {
        Self { items }
    }

    pub fn add(&mut self, item: Item) {
        self.items.push(item);
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn select(&self) -> ItemCollectionFilter {
        ItemCollectionFilter {
            items: self.items.iter().collect()
        }
    } 

}

impl<'a> ItemCollectionFilter<'a> {
    pub fn by_category_alias(&mut self, alias: String) -> &mut Self {
        self.items.retain(|item| item.category.alias == alias);
        self
    }

    pub fn by_month_year(&mut self, month: u32, year: i32) -> &mut Self {
        self.items.retain(|item| {
            item.date.month() == month &&
            item.date.year() == year
        });
        self
    }

    pub fn date_from(&mut self, dt: DateTime<Utc>) -> &mut Self {
        self.items.retain(|item| item.date.timestamp() >= dt.timestamp());
        self
    }

    pub fn date_to(&mut self, dt: DateTime<Utc>) -> &mut Self {
        self.items.retain(|item| item.date.timestamp() < dt.timestamp());
        self
    }

    pub fn get(&self) -> Vec<&'a Item> { 
        self.items.clone()
    }

    pub fn len(&self) -> usize {
        self.items.len() 
    }

    pub fn stat(&self) -> ItemCollectionStat {
        ItemCollectionStat {
            n_items: self.items.len()
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDateTime;
    
    fn parse_dt(str: &str) -> DateTime<Utc> {
        let dt = NaiveDateTime::parse_from_str(str, "%Y-%m-%d %H:%M:%S").unwrap();
        DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc)
    }

    fn get_default_collection() -> ItemCollection {
        let mut collection = ItemCollection::new();

        let category = Category::new("c1".to_string(), "Category 1".to_string());
        collection.add(Item::new(parse_dt("2025-01-01 23:00:00"), category.clone(), 100.0));
        collection.add(Item::new(parse_dt("2025-02-02 23:00:00"), category.clone(), 100.0));
        collection.add(Item::new(parse_dt("2025-03-03 23:00:00"), category.clone(), 100.0));

        let category = Category::new("c2".to_string(), "Category 2".to_string());
        collection.add(Item::new(parse_dt("2025-01-01 23:00:00"), category.clone(), 100.0));
        collection.add(Item::new(parse_dt("2025-02-02 23:00:00"), category.clone(), 100.0));
        collection.add(Item::new(parse_dt("2025-03-03 23:00:00"), category.clone(), 100.0));
        collection
    }

    #[test]
    fn test_collection() {
        let collection = get_default_collection();
        assert_eq!(collection.len(), 6);
    }

    #[test]
    fn test_filter_alias() {
        let collection = get_default_collection();
        let f = collection.select().by_category_alias("c1".to_string()).len();
        assert_eq!(f, 3);
    }

    #[test]
    fn test_filter_date_from() {
        let collection = get_default_collection();
        let f = collection.select().date_from(parse_dt("2025-01-02 00:00:00")).len();
        assert_eq!(f, 4);
    }

    #[test]
    fn test_filter_date_to() {
        let collection = get_default_collection();
        let f = collection
            .select()
            .date_from(parse_dt("2025-02-02 00:00:00"))
            .date_to(parse_dt("2025-03-03 00:00:00"))
            .len();
        assert_eq!(f, 2);
    }

    #[test]
    fn test_filter_by_month() {
        let collection = get_default_collection();
        let f = collection.select().by_month_year(2, 2025).len();
        assert_eq!(f, 2);
    }
}

