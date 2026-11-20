use gtk::{glib, prelude::*, subclass::prelude::*};

fn columns(width: i32, gap: i32, share: i32, left: i32, right: i32) -> Option<(i32, i32)> {
    let space = (width - gap).max(0);
    let a = space * share / 100;
    let b = space - a;
    (a >= left && b >= right).then_some((a, b))
}

mod imp {
    use super::*;
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };

    type ColumnsChanged = Rc<dyn Fn(bool)>;

    #[derive(Default)]
    pub struct Adaptive {
        pub gap: Cell<i32>,
        pub share: Cell<i32>,
        pub equal_height: Cell<bool>,
        pub center_first: Cell<bool>,
        pub columns: Cell<Option<bool>>,
        pub on_columns: RefCell<Option<ColumnsChanged>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Adaptive {
        const NAME: &'static str = "LinRgbAdaptiveLayout";
        type Type = super::Adaptive;
        type ParentType = gtk::LayoutManager;
    }

    impl ObjectImpl for Adaptive {}

    impl Adaptive {
        fn split(&self, width: i32, left: i32, right: i32) -> Option<(i32, i32, i32)> {
            let gap = self.gap.get();
            if self.center_first.get() {
                let x = (width - left) / 2;
                let remaining = width - x - left - gap;
                (x >= 0 && remaining >= right).then_some((x, left, remaining))
            } else {
                columns(width, gap, self.share.get(), left, right).map(|(a, b)| (0, a, b))
            }
        }
    }

    impl LayoutManagerImpl for Adaptive {
        fn request_mode(&self, _: &gtk::Widget) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        fn measure(
            &self,
            widget: &gtk::Widget,
            orientation: gtk::Orientation,
            size: i32,
        ) -> (i32, i32, i32, i32) {
            let (a, b) = visible_children(widget);
            let Some(a) = a else {
                return (0, 0, -1, -1);
            };
            let Some(b) = b else {
                return a.measure(orientation, size);
            };
            let gap = self.gap.get();
            let share = self.share.get();
            let horizontal = gtk::Orientation::Horizontal;
            let (a_min, a_nat, _, _) = a.measure(horizontal, -1);
            let (b_min, b_nat, _, _) = b.measure(horizontal, -1);
            if orientation == horizontal {
                let natural = ((a_nat as u32 * 100).div_ceil(share as u32))
                    .max((b_nat as u32 * 100).div_ceil((100 - share) as u32))
                    as i32;
                return (a_min.max(b_min), natural + gap, -1, -1);
            }
            if size < 0 {
                // An unconstrained width may fit both cards on one row.
                let (a_min, a_nat, _, _) = a.measure(orientation, -1);
                let (b_min, b_nat, _, _) = b.measure(orientation, -1);
                return (a_min.max(b_min), a_nat.max(b_nat), -1, -1);
            }
            if let Some((_, aw, bw)) = self.split(size, a_min, b_min) {
                let (a_min, a_nat, _, _) = a.measure(orientation, aw);
                let (b_min, b_nat, _, _) = b.measure(orientation, bw);
                (a_min.max(b_min), a_nat.max(b_nat), -1, -1)
            } else {
                let (a_min, a_nat, _, _) = a.measure(orientation, size);
                let (b_min, b_nat, _, _) = b.measure(orientation, size);
                (a_min + gap + b_min, a_nat + gap + b_nat, -1, -1)
            }
        }

        fn allocate(&self, widget: &gtk::Widget, width: i32, height: i32, _: i32) {
            let (a, b) = visible_children(widget);
            let Some(a) = a else {
                return;
            };
            let Some(b) = b else {
                a.allocate(width, height, -1, None);
                return;
            };
            let gap = self.gap.get();
            let horizontal = gtk::Orientation::Horizontal;
            let vertical = gtk::Orientation::Vertical;
            let a_min = a.measure(horizontal, -1).0;
            let b_min = b.measure(horizontal, -1).0;
            let split = self.split(width, a_min, b_min);
            if let Some(callback) = self.on_columns.borrow().as_ref() {
                let side_by_side = split.is_some();
                if self.columns.replace(Some(side_by_side)) != Some(side_by_side) {
                    let callback = callback.clone();
                    glib::idle_add_local_once(move || callback(side_by_side));
                }
            }
            if let Some((x, aw, bw)) = split {
                let row_height = |child: &gtk::Widget, width| {
                    if self.equal_height.get() {
                        height
                    } else {
                        child.measure(vertical, width).1.min(height)
                    }
                };
                place(&a, aw, row_height(&a, aw), x, 0);
                place(&b, bw, row_height(&b, bw), x + aw + gap, 0);
            } else {
                let a_height = a.measure(vertical, width).1;
                let b_min = b.measure(vertical, width).0;
                let a_height = a_height.min((height - gap - b_min).max(0));
                a.allocate(width, a_height, -1, None);
                place(
                    &b,
                    width,
                    (height - a_height - gap).max(0),
                    0,
                    a_height + gap,
                );
            }
        }
    }

    fn visible_children(widget: &gtk::Widget) -> (Option<gtk::Widget>, Option<gtk::Widget>) {
        let first = widget.first_child();
        let second = first.as_ref().and_then(WidgetExt::next_sibling);
        let first = first.filter(WidgetExt::should_layout);
        let second = second.filter(WidgetExt::should_layout);
        if first.is_some() {
            (first, second)
        } else {
            (second, None)
        }
    }

    fn place(widget: &gtk::Widget, width: i32, height: i32, x: i32, y: i32) {
        let offset = gtk::graphene::Point::new(x as f32, y as f32);
        widget.allocate(
            width,
            height,
            -1,
            Some(gtk::gsk::Transform::new().translate(&offset)),
        );
    }
}

glib::wrapper! {
    pub struct Adaptive(ObjectSubclass<imp::Adaptive>) @extends gtk::LayoutManager;
}

pub fn pair(gap: i32, left_share: i32) -> gtk::Box {
    let layout = glib::Object::new::<Adaptive>();
    layout.imp().gap.set(gap);
    layout.imp().share.set(left_share);
    let container = gtk::Box::default();
    container.set_layout_manager(Some(layout));
    container
}

pub fn centered_pair(gap: i32) -> gtk::Box {
    let container = pair(gap, 50);
    container
        .layout_manager()
        .and_downcast::<Adaptive>()
        .unwrap()
        .imp()
        .center_first
        .set(true);
    container
}

pub fn on_columns_changed(container: &gtk::Box, callback: impl Fn(bool) + 'static) {
    if let Some(layout) = container.layout_manager().and_downcast::<Adaptive>() {
        layout
            .imp()
            .on_columns
            .replace(Some(std::rc::Rc::new(callback)));
    }
}

pub fn set_equal_height(container: &gtk::Box, equal: bool) {
    if let Some(layout) = container.layout_manager().and_downcast::<Adaptive>()
        && layout.imp().equal_height.replace(equal) != equal
    {
        layout.layout_changed();
    }
}

#[cfg(test)]
mod tests {
    use super::columns;

    #[test]
    fn stacks_before_either_column_would_clip() {
        assert_eq!(columns(812, 12, 40, 310, 400), Some((320, 480)));
        assert_eq!(columns(781, 12, 40, 310, 400), None);
        assert_eq!(columns(360, 9, 50, 170, 170), Some((175, 176)));
        assert_eq!(columns(345, 9, 50, 170, 170), None);
    }
}
