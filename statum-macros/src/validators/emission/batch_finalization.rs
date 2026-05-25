use quote::{format_ident, quote};

pub(super) enum BatchFieldSource<'a> {
    SharedAcrossItems {
        field_builder_chain: &'a proc_macro2::TokenStream,
    },
    PerItemByFn {
        field_builder_chain: &'a proc_macro2::TokenStream,
    },
}

pub(super) enum BatchAsyncMode {
    Sync,
    Async,
}

pub(super) enum BatchFinalizationOperation {
    Build,
    BuildReport,
}

pub(super) struct BatchFinalizationPlan<'a> {
    pub(super) operation: BatchFinalizationOperation,
    pub(super) async_mode: BatchAsyncMode,
    pub(super) field_source: BatchFieldSource<'a>,
}

pub(super) fn generate_batch_finalization(
    plan: BatchFinalizationPlan<'_>,
) -> proc_macro2::TokenStream {
    let builder_method = match plan.operation {
        BatchFinalizationOperation::Build => format_ident!("build"),
        BatchFinalizationOperation::BuildReport => format_ident!("build_report"),
    };

    match (plan.async_mode, plan.field_source) {
        (
            BatchAsyncMode::Sync,
            BatchFieldSource::SharedAcrossItems { field_builder_chain },
        ) => {
            quote! {
                __statum_items
                    .into_iter()
                    .map(|__statum_item| {
                        __statum_item.into_machine()
                            #field_builder_chain
                            .#builder_method()
                    })
                    .collect()
            }
        }
        (
            BatchAsyncMode::Async,
            BatchFieldSource::SharedAcrossItems { field_builder_chain },
        ) => {
            quote! {
                statum::__private::futures::future::join_all(
                    __statum_items.iter().map(|__statum_item| {
                        __statum_item.into_machine()
                            #field_builder_chain
                            .#builder_method()
                    })
                ).await
            }
        }
        (BatchAsyncMode::Sync, BatchFieldSource::PerItemByFn { field_builder_chain }) => {
            quote! {
                let __statum_field_fn = self.__statum_fields_fn;
                self.__statum_items
                    .into_iter()
                    .map(|__statum_item| {
                        let __statum_fields = __statum_field_fn(&__statum_item);
                        __statum_item.into_machine()
                            #field_builder_chain
                            .#builder_method()
                    })
                    .collect()
            }
        }
        (BatchAsyncMode::Async, BatchFieldSource::PerItemByFn { field_builder_chain }) => {
            quote! {
                let __statum_field_fn = &self.__statum_fields_fn;
                statum::__private::futures::future::join_all(
                    self.__statum_items.iter().map(|__statum_item| {
                        let __statum_fields = __statum_field_fn(__statum_item);
                        __statum_item.into_machine()
                            #field_builder_chain
                            .#builder_method()
                    })
                ).await
            }
        }
    }
}
